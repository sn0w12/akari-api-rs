use std::future::Future;

use crate::db::DbPool;
use axum::extract::{FromRef, FromRequestParts};
use axum::http::header;
use axum::http::request::Parts;
use sqlx::FromRow;

use crate::config::Config;
use crate::error::ApiError;

#[derive(Debug, Clone, FromRow)]
pub struct AuthUser {
    pub id: String,
    pub username: String,
    pub display_name: Option<String>,
    pub role: Option<String>,
    pub banned: Option<bool>,
}

/// Authenticated user that is guaranteed to be an admin or owner.
pub struct AdminAuthUser(pub AuthUser);

pub struct OptionalAuthUser(pub Option<AuthUser>);

const SESSION_COOKIE_NAMES: [&str; 2] = [
    "better-auth.session_token",
    "__Secure-better-auth.session_token",
];

fn extract_session_token(headers: &axum::http::HeaderMap) -> Option<String> {
    let cookie_header = headers.get(header::COOKIE)?.to_str().ok()?;
    for cookie in cookie_header.split(';') {
        let cookie = cookie.trim();
        for name in SESSION_COOKIE_NAMES {
            if let Some(value) = cookie.strip_prefix(&format!("{}=", name)) {
                let raw = value.trim();
                return Some(raw.split('.').next().unwrap_or(raw).to_string());
            }
        }
    }
    None
}

impl<S> FromRequestParts<S> for AuthUser
where
    S: Send + Sync,
    AppState: FromRef<S>,
{
    type Rejection = ApiError;

    fn from_request_parts(
        parts: &mut Parts,
        state: &S,
    ) -> impl Future<Output = Result<Self, Self::Rejection>> + Send {
        let app_state = AppState::from_ref(state);
        let token = extract_session_token(&parts.headers);

        async move {
            let token = token.ok_or_else(|| ApiError::Unauthorized {
                message: "No session token".into(),
            })?;

            let user = sqlx::query_as::<_, AuthUser>(
                r#"SELECT u.id, u.name AS username, u."displayUsername" AS display_name, u.role, u.banned
                   FROM auth.user u
                   JOIN auth.session s ON s."userId" = u.id
                   WHERE s.token = $1 AND s."expiresAt" > now()"#,
            )
            .bind(&token)
            .fetch_optional(&app_state.db)
            .await
            .map_err(|e| ApiError::Internal {
                message: format!("Database error: {}", e),
            })?;

            let user = user.ok_or_else(|| ApiError::Unauthorized {
                message: "Invalid or expired session".into(),
            })?;

            if user.banned.unwrap_or(false) {
                return Err(ApiError::Forbidden {
                    message: "User is banned".into(),
                });
            }

            Ok(user)
        }
    }
}

impl<S> FromRequestParts<S> for AdminAuthUser
where
    S: Send + Sync,
    AppState: FromRef<S>,
{
    type Rejection = ApiError;

    fn from_request_parts(
        parts: &mut Parts,
        state: &S,
    ) -> impl Future<Output = Result<Self, Self::Rejection>> + Send {
        async move {
            let user = AuthUser::from_request_parts(parts, state).await?;
            let role = user.role.as_deref().unwrap_or("user");
            if role != "admin" && role != "owner" {
                return Err(ApiError::Forbidden {
                    message: "Admin access required".into(),
                });
            }
            Ok(AdminAuthUser(user))
        }
    }
}

impl<S> FromRequestParts<S> for OptionalAuthUser
where
    S: Send + Sync,
    AppState: FromRef<S>,
{
    type Rejection = ApiError;

    fn from_request_parts(
        parts: &mut Parts,
        state: &S,
    ) -> impl Future<Output = Result<Self, Self::Rejection>> + Send {
        let app_state = AppState::from_ref(state);
        let token = extract_session_token(&parts.headers);

        async move {
            let token = match token {
                Some(t) => t,
                None => return Ok(OptionalAuthUser(None)),
            };

            let user = match sqlx::query_as::<_, AuthUser>(
                r#"SELECT u.id, u.name AS username, u."displayUsername" AS display_name, u.role, u.banned
                   FROM auth.user u
                   JOIN auth.session s ON s."userId" = u.id
                   WHERE s.token = $1 AND s."expiresAt" > now()"#,
            )
            .bind(&token)
            .fetch_optional(&app_state.db)
            .await
            {
                Ok(Some(u)) if !u.banned.unwrap_or(false) => u,
                _ => return Ok(OptionalAuthUser(None)),
            };

            Ok(OptionalAuthUser(Some(user)))
        }
    }
}

#[derive(Clone)]
pub struct AppState {
    pub db: DbPool,
    pub config: Config,
}

impl FromRef<AppState> for DbPool {
    fn from_ref(state: &AppState) -> Self {
        state.db.clone()
    }
}

impl FromRef<AppState> for Config {
    fn from_ref(state: &AppState) -> Self {
        state.config.clone()
    }
}
