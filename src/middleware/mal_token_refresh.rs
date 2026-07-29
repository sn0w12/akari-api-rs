use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll};

use axum::body::Body;
use axum::http::{Request, StatusCode};
use axum::response::Response;
use serde_json::Value;
use sqlx::PgPool;
use tower::{Layer, Service};

use crate::config::Config;

fn base64url_decode(input: &str) -> Option<Vec<u8>> {
    let b64 = input.replace('-', "+").replace('_', "/");
    let b64 = match b64.len() % 4 {
        2 => format!("{}==", b64),
        3 => format!("{}=", b64),
        _ => b64,
    };
    use base64::Engine;
    base64::engine::general_purpose::STANDARD.decode(&b64).ok()
}

fn jwt_expires_within(token: &str, minutes: i64) -> bool {
    let parts: Vec<&str> = token.split('.').collect();
    if parts.len() < 2 {
        return false;
    }
    let decoded =
        match base64url_decode(parts[1]).and_then(|d| serde_json::from_slice::<Value>(&d).ok()) {
            Some(v) => v,
            None => return false,
        };
    let exp = decoded["exp"].as_i64().unwrap_or(0);
    let now = chrono::Utc::now().timestamp();
    exp - now < minutes * 60
}

#[derive(Clone)]
pub struct MalTokenRefreshLayer {
    #[allow(dead_code)]
    pub pool: PgPool,
    pub config: Config,
}

impl<S> Layer<S> for MalTokenRefreshLayer {
    type Service = MalTokenRefreshMiddleware<S>;

    fn layer(&self, inner: S) -> Self::Service {
        MalTokenRefreshMiddleware {
            inner,
            config: self.config.clone(),
        }
    }
}

#[derive(Clone)]
pub struct MalTokenRefreshMiddleware<S> {
    inner: S,
    config: Config,
}

impl<S> Service<Request<Body>> for MalTokenRefreshMiddleware<S>
where
    S: Service<Request<Body>, Response = Response> + Send + Clone + 'static,
    S::Future: Send + 'static,
    S::Error: Into<Box<dyn std::error::Error + Send + Sync>>,
{
    type Response = Response;
    type Error = S::Error;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send>>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx)
    }

    fn call(&mut self, req: Request<Body>) -> Self::Future {
        let path = req.uri().path().to_string();

        let should_refresh =
            path.starts_with("/v2/mal/") && !path.ends_with("/token") && !path.ends_with("/logout");

        if !should_refresh {
            return Box::pin(self.inner.call(req));
        }

        let cookie_header = req
            .headers()
            .get(axum::http::header::COOKIE)
            .and_then(|v| v.to_str().ok())
            .map(|s| s.to_string())
            .unwrap_or_default();

        let access_token = parse_cookie(&cookie_header, "mal_access_token");
        let refresh_token = parse_cookie(&cookie_header, "mal_refresh_token");

        let needs_refresh = access_token
            .as_ref()
            .map(|t| jwt_expires_within(t, 5))
            .unwrap_or(true);

        if !needs_refresh || refresh_token.is_none() {
            return Box::pin(self.inner.call(req));
        }

        let config = self.config.clone();
        let mut inner = self.inner.clone();
        let refresh_token = refresh_token.unwrap();

        Box::pin(async move {
            let client = reqwest::Client::new();
            let params = [
                ("client_id", config.mal_client_id.as_str()),
                ("grant_type", "refresh_token"),
                ("refresh_token", &refresh_token),
            ];

            match client
                .post("https://myanimelist.net/v1/oauth2/token")
                .form(&params)
                .send()
                .await
            {
                Ok(resp) if resp.status().is_success() => {
                    let text = resp.text().await.unwrap_or_default();
                    let data: Value = serde_json::from_str(&text).unwrap_or_default();
                    let new_access = data["access_token"].as_str().unwrap_or("");
                    let new_refresh = data["refresh_token"].as_str().unwrap_or("");
                    let expires_in = data["expires_in"].as_i64().unwrap_or(0);

                    if !new_access.is_empty() {
                        let mut resp = inner.call(req).await?;

                        let access_cookie = format!(
                            "mal_access_token={}; Path=/; Max-Age={}; SameSite=Lax",
                            new_access, expires_in
                        );
                        let refresh_cookie = format!(
                            "mal_refresh_token={}; Path=/; Max-Age=2678400; SameSite=Lax",
                            new_refresh
                        );

                        resp.headers_mut().insert(
                            axum::http::header::SET_COOKIE,
                            access_cookie.parse().unwrap(),
                        );
                        resp.headers_mut().append(
                            axum::http::header::SET_COOKIE,
                            refresh_cookie.parse().unwrap(),
                        );

                        return Ok(resp);
                    }
                }
                Ok(resp)
                    if resp.status() == StatusCode::UNAUTHORIZED
                        || resp.status() == StatusCode::BAD_REQUEST =>
                {
                    let mut resp = Response::new(Body::empty());
                    *resp.status_mut() = StatusCode::UNAUTHORIZED;
                    resp.headers_mut().insert(
                        axum::http::header::SET_COOKIE,
                        "mal_access_token=; Path=/; Max-Age=0".parse().unwrap(),
                    );
                    resp.headers_mut().append(
                        axum::http::header::SET_COOKIE,
                        "mal_refresh_token=; Path=/; Max-Age=0".parse().unwrap(),
                    );
                    return Ok(resp);
                }
                _ => {}
            }

            inner.call(req).await
        })
    }
}

fn parse_cookie(cookie_header: &str, name: &str) -> Option<String> {
    for part in cookie_header.split(';') {
        let part = part.trim();
        if let Some(value) = part.strip_prefix(&format!("{}=", name)) {
            return Some(value.to_string());
        }
    }
    None
}
