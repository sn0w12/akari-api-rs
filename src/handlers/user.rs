use axum::Json;
use axum::extract::{Path, Query, State};
use chrono::{DateTime, Utc};
use serde::Deserialize;
use uuid::Uuid;

use crate::auth::AuthUser;
use crate::db::DbPool;
use crate::error::{ApiError, ErrorResponseTemplate};
use crate::models::user::{UserListResponse, UserProfileDetailsResponse, UserResponse, UserRole};
use crate::response::SuccessResponse;

#[derive(Debug, Clone, Deserialize, utoipa::IntoParams)]
pub struct UserListParams {
    pub page: Option<i32>,
    pub page_size: Option<i32>,
}

#[derive(Debug, sqlx::FromRow)]
#[allow(dead_code)]
struct UserProfileRow {
    id: String,
    name: String,
    #[sqlx(rename = "displayUsername")]
    display_username: Option<String>,
    role: Option<String>,
    banned: Option<bool>,
    image: Option<String>,
    created_at: DateTime<Utc>,
    total_comments: i64,
    total_upvotes: i64,
    total_bookmarks: i64,
    total_lists: i64,
}

/// GET /v2/user/{id}/profile
#[utoipa::path(get, path = "/v2/user/{userId}", tag = "user", responses(
    (status = 200, description = "Success", body = SuccessResponse<UserProfileDetailsResponse>),
    (status = 404, description = "Not found", body = ErrorResponseTemplate),
    (status = 500, description = "Internal error", body = ErrorResponseTemplate),
))]
pub async fn user_profile(
    Path(user_id): Path<String>,
    State(db): State<DbPool>,
) -> Result<Json<SuccessResponse<UserProfileDetailsResponse>>, ApiError> {
    let row = sqlx::query_as::<_, UserProfileRow>(
        "SELECT u.id, u.name, u.\"displayUsername\", u.role, u.banned, u.image, u.\"createdAt\" AS created_at, \
                (SELECT COUNT(*) FROM public.comments WHERE user_id = u.id AND deleted = FALSE)::bigint AS total_comments, \
                (SELECT COUNT(*) FROM public.comment_votes cv JOIN public.comments c ON c.id = cv.comment_id WHERE c.user_id = u.id AND cv.value > 0)::bigint AS total_upvotes, \
                0::bigint AS total_bookmarks, \
                0::bigint AS total_lists \
         FROM auth.user u \
         WHERE u.id = $1",
    )
    .bind(&user_id)
    .fetch_optional(&db)
    .await?
    .ok_or(ApiError::not_found("User not found"))?;

    Ok(Json(SuccessResponse {
        result: "Success".to_string(),
        status: 200,
        data: UserProfileDetailsResponse {
            user_id: row.id,
            display_name: row.display_username.clone().unwrap_or(row.name.clone()),
            username: row.name,
            role: row
                .role
                .as_deref()
                .map(|r| match r {
                    "admin" => UserRole::Admin,
                    "owner" => UserRole::Owner,
                    _ => UserRole::User,
                })
                .unwrap_or(UserRole::User),
            banned: row.banned.unwrap_or(false),
            created_at: Some(row.created_at),
            total_comments: Some(row.total_comments),
            total_upvotes: Some(row.total_upvotes),
            total_downvotes: Some(0),
            total_bookmarks: Some(row.total_bookmarks),
            total_uploads: Some(0),
            total_lists: Some(row.total_lists),
        },
    }))
}

/// GET /v2/user/list
#[utoipa::path(get, path = "/v2/user", tag = "user", params(UserListParams), responses(
    (status = 200, description = "Success", body = SuccessResponse<UserListResponse>),
    (status = 500, description = "Internal error", body = ErrorResponseTemplate),
))]
pub async fn list_users(
    Query(params): Query<UserListParams>,
    State(db): State<DbPool>,
) -> Result<Json<SuccessResponse<UserListResponse>>, ApiError> {
    let page = params.page.unwrap_or(1).max(1);
    let page_size = params.page_size.unwrap_or(20).clamp(1, 100);
    let offset = ((page - 1) as i64) * (page_size as i64);

    let total_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM auth.user")
        .fetch_one(&db)
        .await?;

    let total_pages = ((total_count as f64) / (page_size as f64)).ceil() as i32;

    #[derive(Debug, sqlx::FromRow)]
    struct ListRow {
        id: String,
        name: String,
        #[sqlx(rename = "displayUsername")]
        display_username: Option<String>,
        role: Option<String>,
        banned: Option<bool>,
        created_at: DateTime<Utc>,
        total_comments: i64,
        total_upvotes: i64,
    }

    let rows: Vec<ListRow> = sqlx::query_as::<_, ListRow>(
        "SELECT u.id, u.name, u.\"displayUsername\", u.role, u.banned, u.\"createdAt\" AS created_at, \
                (SELECT COUNT(*) FROM public.comments WHERE user_id = u.id AND deleted = FALSE)::bigint AS total_comments, \
                (SELECT COUNT(*) FROM public.comment_votes cv JOIN public.comments c ON c.id = cv.comment_id WHERE c.user_id = u.id AND cv.value > 0)::bigint AS total_upvotes \
         FROM auth.user u \
         ORDER BY u.\"createdAt\" DESC \
         LIMIT $1 OFFSET $2",
    )
    .bind(page_size as i64)
    .bind(offset)
    .fetch_all(&db)
    .await?;

    let items: Vec<UserProfileDetailsResponse> = rows
        .into_iter()
        .map(|r| UserProfileDetailsResponse {
            user_id: r.id,
            display_name: r.display_username.clone().unwrap_or(r.name.clone()),
            username: r.name,
            role: r
                .role
                .as_deref()
                .map(|r| match r {
                    "admin" => UserRole::Admin,
                    "owner" => UserRole::Owner,
                    _ => UserRole::User,
                })
                .unwrap_or(UserRole::User),
            banned: r.banned.unwrap_or(false),
            created_at: Some(r.created_at),
            total_comments: Some(r.total_comments),
            total_upvotes: Some(r.total_upvotes),
            total_downvotes: Some(0),
            total_bookmarks: Some(0),
            total_uploads: Some(0),
            total_lists: Some(0),
        })
        .collect();

    Ok(Json(SuccessResponse {
        result: "Success".to_string(),
        status: 200,
        data: UserListResponse {
            items,
            total_items: total_count,
            current_page: page,
            page_size,
            total_pages,
        },
    }))
}

#[derive(Debug, Deserialize, utoipa::ToSchema, utoipa::IntoParams)]
pub struct ProfileUpdateBody {
    pub username: Option<String>,
    #[serde(rename = "displayName")]
    pub display_name: Option<String>,
}

/// GET /v2/user/me
#[utoipa::path(get, path = "/v2/user/me", tag = "user", responses(
    (status = 200, description = "Success", body = SuccessResponse<UserResponse>),
    (status = 401, description = "Unauthorized", body = ErrorResponseTemplate),
    (status = 500, description = "Internal error", body = ErrorResponseTemplate),
))]
pub async fn me(user: AuthUser) -> Result<Json<SuccessResponse<UserResponse>>, ApiError> {
    Ok(Json(SuccessResponse {
        result: "Success".to_string(),
        status: 200,
        data: UserResponse {
            user_id: user.id,
            username: user.username.clone(),
            display_name: user.display_name.clone().unwrap_or(user.username.clone()),
            role: user
                .role
                .as_deref()
                .map(|r| match r {
                    "admin" => UserRole::Admin,
                    "owner" => UserRole::Owner,
                    _ => UserRole::User,
                })
                .unwrap_or(UserRole::User),
            banned: user.banned.unwrap_or(false),
        },
    }))
}

/// PUT /v2/user/profile
#[utoipa::path(put, path = "/v2/user/profile", tag = "user", responses(
    (status = 200, description = "Success"),
    (status = 400, description = "Bad request", body = ErrorResponseTemplate),
    (status = 401, description = "Unauthorized", body = ErrorResponseTemplate),
    (status = 500, description = "Internal error", body = ErrorResponseTemplate),
))]
pub async fn update_profile(
    user: AuthUser,
    State(db): State<DbPool>,
    Json(body): Json<ProfileUpdateBody>,
) -> Result<Json<SuccessResponse<serde_json::Value>>, ApiError> {
    if let Some(ref username) = body.username {
        let username = username.trim().to_lowercase();
        if username.len() < 2 || username.len() > 100 {
            return Err(ApiError::bad_request("Username must be 2-100 characters"));
        }
        if !username.chars().all(|c| c.is_alphanumeric() || c == '-') {
            return Err(ApiError::bad_request(
                "Username must be alphanumeric with dashes",
            ));
        }
        if Uuid::parse_str(&username).is_ok() {
            return Err(ApiError::bad_request("Username cannot be a UUID"));
        }

        // Check uniqueness
        let exists: bool = sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM auth.user WHERE username = $1 AND id != $2)",
        )
        .bind(&username)
        .bind(&user.id)
        .fetch_one(&db)
        .await?;

        if exists {
            return Err(ApiError::bad_request("Username already taken"));
        }

        sqlx::query("UPDATE auth.user SET username = $1 WHERE id = $2")
            .bind(&username)
            .bind(&user.id)
            .execute(&db)
            .await?;
    }

    if let Some(ref display_name) = body.display_name {
        let name = display_name.trim();
        if !name.is_empty() {
            sqlx::query("UPDATE auth.user SET \"displayUsername\" = $1 WHERE id = $2")
                .bind(name)
                .bind(&user.id)
                .execute(&db)
                .await?;
        }
    }

    Ok(Json(SuccessResponse {
        result: "Success".to_string(),
        status: 200,
        data: serde_json::json!({}),
    }))
}
