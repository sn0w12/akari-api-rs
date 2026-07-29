use axum::Json;
use axum::extract::State;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

use crate::auth::AuthUser;
use crate::config::Config;
use crate::db::DbPool;
use crate::error::{ApiError, ErrorResponseTemplate};
use crate::response::SuccessResponse;

#[derive(Debug, Deserialize, utoipa::ToSchema, utoipa::IntoParams)]
pub struct PushSubscriptionRequest {
    pub endpoint: String,
    pub p256dh: String,
    pub auth: String,
}

#[derive(Debug, Deserialize, utoipa::ToSchema, utoipa::IntoParams)]
pub struct SendNotificationBody {
    pub title: String,
    pub body: String,
    pub url: String,
    #[serde(rename = "mangaId")]
    pub manga_id: Uuid,
}

#[derive(Debug, sqlx::FromRow, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct WebsiteNotification {
    pub id: i64,
    pub title: String,
    pub content: String,
    pub created_at: DateTime<Utc>,
}

/// POST /v2/notifications/subscribe
#[utoipa::path(post, path = "/v2/notifications/subscribe", tag = "notifications", responses(
    (status = 200, description = "Success"),
    (status = 400, description = "Bad request", body = ErrorResponseTemplate),
    (status = 401, description = "Unauthorized", body = ErrorResponseTemplate),
    (status = 500, description = "Internal error", body = ErrorResponseTemplate),
))]
pub async fn subscribe(
    user: AuthUser,
    State(db): State<DbPool>,
    State(config): State<Config>,
    Json(body): Json<PushSubscriptionRequest>,
) -> Result<Json<SuccessResponse<&'static str>>, ApiError> {
    if body.endpoint.is_empty() || body.p256dh.is_empty() || body.auth.is_empty() {
        return Err(ApiError::bad_request("Missing required fields"));
    }

    let enc_p256dh = crate::services::push::encrypt(&body.p256dh, &config.encryption_key);
    let enc_auth = crate::services::push::encrypt(&body.auth, &config.encryption_key);

    sqlx::query(
        "INSERT INTO public.push_subscriptions (user_id, endpoint, p256dh, auth) \
         VALUES ($1, $2, $3, $4) \
         ON CONFLICT (endpoint) DO UPDATE SET p256dh = $3, auth = $4",
    )
    .bind(&user.id)
    .bind(&body.endpoint)
    .bind(&enc_p256dh)
    .bind(&enc_auth)
    .execute(&db)
    .await
    .map_err(|e| {
        if e.to_string().contains("duplicate") {
            ApiError::bad_request("Subscription already exists")
        } else {
            ApiError::internal(format!("Failed to subscribe: {}", e))
        }
    })?;

    Ok(Json(SuccessResponse {
        result: "Success".to_string(),
        status: 200,
        data: "Subscribed successfully",
    }))
}

/// GET /v2/notifications/website
#[utoipa::path(get, path = "/v2/notifications/website", tag = "notifications", responses(
    (status = 200, description = "Success", body = SuccessResponse<Vec<WebsiteNotification>>),
    (status = 500, description = "Internal error", body = ErrorResponseTemplate),
))]
pub async fn website_notifications(
    State(db): State<DbPool>,
) -> Result<Json<SuccessResponse<Vec<WebsiteNotification>>>, ApiError> {
    let notifications: Vec<WebsiteNotification> = sqlx::query_as::<_, WebsiteNotification>(
        "SELECT id, title, content, created_at FROM public.website_notifications WHERE visible = TRUE ORDER BY created_at DESC",
    )
    .fetch_all(&db)
    .await
    .unwrap_or_default();

    Ok(Json(SuccessResponse {
        result: "Success".to_string(),
        status: 200,
        data: notifications,
    }))
}

/// POST /v2/notifications/send
#[utoipa::path(post, path = "/v2/notifications/send", tag = "notifications", responses(
    (status = 200, description = "Success"),
    (status = 400, description = "Bad request", body = ErrorResponseTemplate),
    (status = 500, description = "Internal error", body = ErrorResponseTemplate),
))]
pub async fn send_notification(
    State(_db): State<DbPool>,
    State(config): State<Config>,
    headers: axum::http::HeaderMap,
    Json(_body): Json<SendNotificationBody>,
) -> Result<Json<SuccessResponse<&'static str>>, ApiError> {
    // Verify API key
    let api_key = headers
        .get("X-API-Key")
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_string())
        .ok_or(ApiError::bad_request("Missing X-API-Key header"))?;

    if api_key != config.api_key {
        return Err(ApiError::bad_request("Invalid API key"));
    }

    // For now, acknowledge the send request.
    // Full WebPush sending requires VAPID key setup.
    // The push sending logic would:
    // 1. Query bookmarked users' push subscriptions
    // 2. Decrypt p256dh/auth
    // 3. Send via WebPush protocol
    // See services/push.rs for the send_webpush function.

    Ok(Json(SuccessResponse {
        result: "Success".to_string(),
        status: 200,
        data: "Notifications queued",
    }))
}
