use axum::Json;
use axum::extract::State;
use chrono::{DateTime, Utc};
use futures::stream::StreamExt;
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

/// A push subscription belonging to a user who bookmarked the target manga,
/// with that user's unread bookmarks count (used as the notification badge).
#[derive(Debug, sqlx::FromRow)]
struct PushSubscriptionRow {
    endpoint: String,
    p256dh: String,
    auth: String,
    unread_count: i32,
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
    (status = 401, description = "Unauthorized", body = ErrorResponseTemplate),
    (status = 500, description = "Internal error", body = ErrorResponseTemplate),
))]
pub async fn send_notification(
    State(db): State<DbPool>,
    State(config): State<Config>,
    headers: axum::http::HeaderMap,
    Json(body): Json<SendNotificationBody>,
) -> Result<Json<SuccessResponse<&'static str>>, ApiError> {
    // Verify API key
    let api_key = headers
        .get("X-API-Key")
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_string())
        .ok_or_else(|| ApiError::Unauthorized {
            message: "Invalid API key".to_string(),
        })?;

    if api_key != config.api_key {
        return Err(ApiError::Unauthorized {
            message: "Invalid API key".to_string(),
        });
    }

    let vapid = crate::services::push::VapidKeys::parse(
        &config.vapid_subject,
        &config.vapid_public_key,
        &config.vapid_private_key,
    )
    .map_err(ApiError::internal)?;

    let subscriptions: Vec<PushSubscriptionRow> = sqlx::query_as::<_, PushSubscriptionRow>(
        "SELECT DISTINCT \
           ps.endpoint, ps.p256dh, ps.auth, \
           COALESCE(( \
             SELECT COUNT(*) FROM public.user_library_entries ule \
             WHERE ule.user_id = ps.user_id AND EXISTS ( \
               SELECT 1 FROM public.chapters ch \
               WHERE ch.work_id = ule.work_id \
                 AND ch.number > (SELECT COALESCE(c.number, 0) FROM public.chapters c WHERE c.id = ule.last_read_chapter_id) \
             ) \
           ), 0)::integer AS unread_count \
         FROM public.push_subscriptions ps \
         INNER JOIN public.user_library_entries ub ON ps.user_id = ub.user_id \
         WHERE ub.work_id = $1",
    )
    .bind(body.manga_id)
    .fetch_all(&db)
    .await?;

    if subscriptions.is_empty() {
        return Ok(Json(SuccessResponse {
            result: "Success".to_string(),
            status: 200,
            data: "Notifications sent successfully",
        }));
    }

    let http = reqwest::Client::new();
    let expired: std::sync::Mutex<Vec<String>> = std::sync::Mutex::new(Vec::new());

    let title = body.title.clone();
    let body_text = body.body.clone();
    let url = body.url.clone();
    let manga_id_str = body.manga_id.to_string();
    let tag = format!("manga-{}", body.manga_id);

    // Deliver to every subscription concurrently; collect dead ones.
    futures::stream::iter(subscriptions)
        .map(|row| {
            let http = &http;
            let expired = &expired;
            let vapid = &vapid;
            let enc_key = &config.encryption_key;
            let payload = serde_json::json!({
                "title": title,
                "body": body_text,
                "url": url,
                "mangaId": manga_id_str,
                "tag": tag,
                "badge": row.unread_count,
            })
            .to_string()
            .into_bytes();
            async move {
                let p256dh = match crate::services::push::decrypt(&row.p256dh, enc_key) {
                    Ok(v) => v,
                    Err(e) => {
                        tracing::warn!(endpoint = %row.endpoint, error = %e, "failed to decrypt p256dh, skipping");
                        return;
                    }
                };
                let auth = match crate::services::push::decrypt(&row.auth, enc_key) {
                    Ok(v) => v,
                    Err(e) => {
                        tracing::warn!(endpoint = %row.endpoint, error = %e, "failed to decrypt auth, skipping");
                        return;
                    }
                };

                match crate::services::push::send_webpush(
                    http,
                    &row.endpoint,
                    &p256dh,
                    &auth,
                    &payload,
                    vapid,
                )
                .await
                {
                    Ok(()) => tracing::debug!(endpoint = %row.endpoint, "push notification sent"),
                    Err(crate::services::push::PushError::Expired) => {
                        tracing::info!(endpoint = %row.endpoint, "push subscription expired, removing");
                        expired.lock().unwrap().push(row.endpoint);
                    }
                    Err(e) => {
                        tracing::warn!(endpoint = %row.endpoint, error = %e, "push delivery failed");
                    }
                }
            }
        })
        .buffer_unordered(16)
        .collect::<Vec<_>>()
        .await;

    // Clean up subscriptions the push service reported as gone (404/410).
    let expired = std::mem::take(&mut *expired.lock().unwrap());
    if !expired.is_empty() {
        sqlx::query("DELETE FROM public.push_subscriptions WHERE endpoint = ANY($1)")
            .bind(&expired)
            .execute(&db)
            .await?;
    }

    Ok(Json(SuccessResponse {
        result: "Success".to_string(),
        status: 200,
        data: "Notifications sent successfully",
    }))
}
