use std::collections::HashMap;
use std::future::Future;
use std::pin::Pin;
use std::sync::{Arc, Mutex};
use std::task::{Context, Poll};
use std::time::Instant;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use axum::response::Response;
use tower::{Layer, Service};

use crate::config::Config;

/// Simple fixed-window rate limiter per IP.
/// Global: 20 requests per 5 seconds per IP.
/// Requests with valid X-API-Key bypass rate limiting.

#[derive(Clone)]
pub struct RateLimitLayer {
    pub config: Config,
}

impl<S> Layer<S> for RateLimitLayer {
    type Service = RateLimitMiddleware<S>;

    fn layer(&self, inner: S) -> Self::Service {
        RateLimitMiddleware {
            inner,
            config: self.config.clone(),
            state: Arc::new(Mutex::new(RateLimitState {
                windows: HashMap::new(),
            })),
        }
    }
}

#[derive(Clone)]
pub struct RateLimitMiddleware<S> {
    inner: S,
    config: Config,
    state: Arc<Mutex<RateLimitState>>,
}

struct RateLimitState {
    windows: HashMap<String, FixedWindow>,
}

struct FixedWindow {
    start: Instant,
    count: u32,
}

const LIMIT: u32 = 20;
const WINDOW_SECS: u64 = 5;

impl<S> Service<Request<Body>> for RateLimitMiddleware<S>
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
        let bypass = req
            .headers()
            .get("X-API-Key")
            .and_then(|v| v.to_str().ok())
            .map(|k| k == self.config.api_key)
            .unwrap_or(false);

        if bypass {
            return Box::pin(self.inner.call(req));
        }

        let ip = req
            .headers()
            .get("X-Forwarded-For")
            .and_then(|v| v.to_str().ok())
            .and_then(|s| s.split(',').next())
            .map(|s| s.trim().to_string())
            .unwrap_or_else(|| "unknown".to_string());

        let mut state = self.state.lock().unwrap();
        let now = Instant::now();
        let window = state.windows.entry(ip.clone()).or_insert(FixedWindow {
            start: now,
            count: 0,
        });

        if now.duration_since(window.start).as_secs() >= WINDOW_SECS {
            window.start = now;
            window.count = 0;
        }

        window.count += 1;

        if window.count > LIMIT {
            let mut resp = Response::new(Body::empty());
            *resp.status_mut() = StatusCode::TOO_MANY_REQUESTS;
            return Box::pin(async move { Ok(resp) });
        }

        Box::pin(self.inner.call(req))
    }
}
