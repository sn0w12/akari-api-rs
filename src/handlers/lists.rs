use axum::extract::{Path, Query, State};
use axum::Json;
use chrono::{DateTime, Utc};
use serde::Deserialize;
use uuid::Uuid;

use crate::auth::{AuthUser, OptionalAuthUser};
use crate::db::DbPool;
use crate::error::{ApiError, ErrorResponseTemplate};
use crate::models::list::{
    CreateListBody, ListEntryResponse, UpdateEntryBody, UserListDetailResponse, UserListResponse,
};
use crate::models::user::{UserResponse, UserRole};
use crate::response::{ItemsResponse, PaginatedResponse, SuccessResponse};

#[derive(Debug, Clone, Deserialize, utoipa::IntoParams)]
#[serde(rename_all = "camelCase")]
pub struct ListParams {
    pub page: Option<i32>,
    pub page_size: Option<i32>,
}

fn pagination(page: Option<i32>, page_size: Option<i32>) -> (i32, i32, i64) {
    let p = page.unwrap_or(1).max(1);
    let ps = page_size.unwrap_or(20).clamp(1, 100);
    (p, ps, ((p - 1) as i64) * (ps as i64))
}

#[derive(Debug, sqlx::FromRow)]
struct ListRow {
    id: Uuid,
    user_id: String,
    title: String,
    description: Option<String>,
    is_public: bool,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
    total_entries: i64,
}

/// GET /v2/lists/user/{user_id}
#[utoipa::path(get, path = "/v2/lists/user/{userId}", tag = "lists", params(ListParams), responses(
    (status = 200, description = "Success", body = SuccessResponse<PaginatedResponse<UserListResponse>>),
    (status = 500, description = "Internal error", body = ErrorResponseTemplate),
))]
pub async fn list_user_lists(
    Path(user_id): Path<String>,
    Query(params): Query<ListParams>,
    State(db): State<DbPool>,
) -> Result<Json<SuccessResponse<PaginatedResponse<UserListResponse>>>, ApiError> {
    let (page, page_size, offset) = pagination(params.page, params.page_size);
    let total_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM public.user_lists WHERE user_id = $1 AND is_public = TRUE",
    )
    .bind(&user_id)
    .fetch_one(&db).await?;
    let total_pages = ((total_count as f64) / (page_size as f64)).ceil() as i32;
    let rows: Vec<ListRow> = sqlx::query_as::<_, ListRow>(
        "SELECT ul.id, ul.user_id, ul.title, ul.description, ul.is_public, ul.created_at, ul.updated_at, \
         (SELECT COUNT(*) FROM public.user_list_entries ule WHERE ule.list_id = ul.id)::bigint AS total_entries \
         FROM public.user_lists ul \
         WHERE ul.user_id = $1 AND ul.is_public = TRUE \
         ORDER BY ul.updated_at DESC LIMIT $2 OFFSET $3",
    )
    .bind(&user_id).bind(page_size as i64).bind(offset)
    .fetch_all(&db).await?;
    let items: Vec<UserListResponse> = rows.into_iter().map(|r| UserListResponse {
        id: r.id, user_id: r.user_id, title: r.title, description: r.description,
        is_public: r.is_public, created_at: r.created_at, updated_at: r.updated_at,
        total_entries: r.total_entries as i32,
    }).collect();
    Ok(Json(SuccessResponse { result: "Success".to_string(), status: 200, data: PaginatedResponse {
        items, total_items: total_count, current_page: page, page_size, total_pages,
    } }))
}

/// GET /v2/lists/user/me
#[utoipa::path(get, path = "/v2/lists/user/me", tag = "lists", params(ListParams), responses(
    (status = 200, description = "Success", body = SuccessResponse<ItemsResponse<UserListResponse>>),
    (status = 401, description = "Unauthorized", body = ErrorResponseTemplate),
    (status = 500, description = "Internal error", body = ErrorResponseTemplate),
))]
pub async fn list_my_lists(
    user: AuthUser,
    Query(params): Query<ListParams>,
    State(db): State<DbPool>,
) -> Result<Json<SuccessResponse<ItemsResponse<UserListResponse>>>, ApiError> {
    let (_page, page_size, offset) = pagination(params.page, params.page_size);
    let rows: Vec<ListRow> = sqlx::query_as::<_, ListRow>(
        "SELECT ul.id, ul.user_id, ul.title, ul.description, ul.is_public, ul.created_at, ul.updated_at, \
         (SELECT COUNT(*) FROM public.user_list_entries ule WHERE ule.list_id = ul.id)::bigint AS total_entries \
         FROM public.user_lists ul \
         WHERE ul.user_id = $1 \
         ORDER BY ul.updated_at DESC LIMIT $2 OFFSET $3",
    )
    .bind(&user.id).bind(page_size as i64).bind(offset)
    .fetch_all(&db).await?;
    let items: Vec<UserListResponse> = rows.into_iter().map(|r| UserListResponse {
        id: r.id, user_id: r.user_id, title: r.title, description: r.description,
        is_public: r.is_public, created_at: r.created_at, updated_at: r.updated_at,
        total_entries: r.total_entries as i32,
    }).collect();
    Ok(Json(SuccessResponse { result: "Success".to_string(), status: 200, data: ItemsResponse { items } }))
}

/// GET /v2/lists/{id}
#[utoipa::path(get, path = "/v2/lists/{id}", tag = "lists", responses(
    (status = 200, description = "Success", body = SuccessResponse<UserListDetailResponse>),
    (status = 404, description = "Not found", body = ErrorResponseTemplate),
    (status = 500, description = "Internal error", body = ErrorResponseTemplate),
))]
pub async fn get_list(
    Path(list_id): Path<Uuid>,
    OptionalAuthUser(user): OptionalAuthUser,
    State(db): State<DbPool>,
) -> Result<Json<SuccessResponse<UserListDetailResponse>>, ApiError> {
    #[derive(Debug, sqlx::FromRow)]
    struct ListOwnerRow {
        id: Uuid, user_id: String, title: String, description: Option<String>,
        is_public: bool, created_at: DateTime<Utc>, updated_at: DateTime<Utc>,
        total_entries: i64, owner_name: String, owner_display: Option<String>,
        owner_role: Option<String>, owner_banned: Option<bool>,
    }
    let list = sqlx::query_as::<_, ListOwnerRow>(
        "SELECT ul.id, ul.user_id, ul.title, ul.description, ul.is_public, ul.created_at, ul.updated_at, \
         (SELECT COUNT(*) FROM public.user_list_entries ule WHERE ule.list_id = ul.id)::bigint AS total_entries, \
         u.name AS owner_name, u.\"displayUsername\" AS owner_display, u.role AS owner_role, u.banned AS owner_banned \
         FROM public.user_lists ul \
         JOIN auth.user u ON u.id = ul.user_id \
         WHERE ul.id = $1",
    )
    .bind(list_id).fetch_optional(&db).await?
    .ok_or(ApiError::not_found("List not found"))?;

    let is_owner = user.as_ref().map(|u| u.id == list.user_id).unwrap_or(false);
    if !list.is_public && !is_owner {
        return Err(ApiError::not_found("List not found"));
    }

    let entries: Vec<ListEntryResponse> = sqlx::query_as::<_, ListEntryRow>(
        "SELECT ule.list_id, ule.work_id, ule.order_index, ule.created_at, ule.updated_at, \
         w.title AS manga_title, cov.url AS manga_cover, w.description AS manga_description \
         FROM public.user_list_entries ule \
         JOIN public.works w ON w.id = ule.work_id \
         LEFT JOIN LATERAL (SELECT url FROM public.covers WHERE work_id = w.id AND is_preferred = TRUE LIMIT 1) cov ON TRUE \
         WHERE ule.list_id = $1 \
         ORDER BY ule.order_index ASC",
    )
    .bind(list_id).fetch_all(&db).await?
    .into_iter().map(|r| ListEntryResponse {
        id: r.list_id, list_id: r.list_id, work_id: r.work_id,
        order_index: r.order_index,
        created_at: r.created_at, updated_at: r.updated_at,
        manga_title: r.manga_title, manga_cover: r.manga_cover.unwrap_or_default(), manga_description: r.manga_description,
    }).collect();

    let uid = list.user_id.clone();
    let uname = list.owner_name.clone();
    Ok(Json(SuccessResponse {
        result: "Success".to_string(), status: 200,
        data: UserListDetailResponse {
            id: list.id, user_id: uid.clone(), title: list.title,
            description: list.description, is_public: list.is_public,
            created_at: list.created_at, updated_at: list.updated_at,
            total_entries: list.total_entries as i32, entries,
            user: UserResponse {
                user_id: uid,
                username: uname.clone(),
                display_name: list.owner_display.unwrap_or(uname),
                role: list.owner_role.as_deref().map(|r| match r {
                    "admin" => UserRole::Admin,
                    "owner" => UserRole::Owner,
                    _ => UserRole::User,
                }).unwrap_or(UserRole::User),
                banned: list.owner_banned.unwrap_or(false),
            },
        },
    }))
}

#[derive(Debug, sqlx::FromRow)]
struct ListEntryRow {
    list_id: Uuid, work_id: Uuid, order_index: i32,
    created_at: DateTime<Utc>, updated_at: DateTime<Utc>,
    manga_title: String, manga_cover: Option<String>, manga_description: String,
}

/// POST /v2/lists
#[utoipa::path(post, path = "/v2/lists", tag = "lists", responses(
    (status = 200, description = "Success", body = SuccessResponse<UserListResponse>),
    (status = 401, description = "Unauthorized", body = ErrorResponseTemplate),
    (status = 500, description = "Internal error", body = ErrorResponseTemplate),
))]
pub async fn create_list(
    user: AuthUser,
    State(db): State<DbPool>,
    Json(body): Json<CreateListBody>,
) -> Result<Json<SuccessResponse<UserListResponse>>, ApiError> {
    let row: ListRow = sqlx::query_as::<_, ListRow>(
        "INSERT INTO public.user_lists (user_id, title, description, is_public) \
         VALUES ($1, $2, $3, COALESCE($4, FALSE)) \
         RETURNING id, user_id, title, description, is_public, created_at, updated_at, 0::bigint AS total_entries",
    )
    .bind(&user.id).bind(&body.title).bind(&body.description).bind(body.is_public)
    .fetch_one(&db).await?;
    Ok(Json(SuccessResponse { result: "Success".to_string(), status: 201, data: UserListResponse {
        id: row.id, user_id: row.user_id, title: row.title, description: row.description,
        is_public: row.is_public, created_at: row.created_at, updated_at: row.updated_at,
        total_entries: 0,
    } }))
}

/// DELETE /v2/lists/{id}
#[utoipa::path(delete, path = "/v2/lists/{id}", tag = "lists", responses(
    (status = 200, description = "Success"),
    (status = 401, description = "Unauthorized", body = ErrorResponseTemplate),
    (status = 404, description = "Not found", body = ErrorResponseTemplate),
    (status = 500, description = "Internal error", body = ErrorResponseTemplate),
))]
pub async fn delete_list(
    user: AuthUser, Path(list_id): Path<Uuid>, State(db): State<DbPool>,
) -> Result<Json<SuccessResponse<serde_json::Value>>, ApiError> {
    let r = sqlx::query("DELETE FROM public.user_lists WHERE id = $1 AND user_id = $2")
        .bind(list_id).bind(&user.id).execute(&db).await?;
    if r.rows_affected() == 0 { return Err(ApiError::not_found("List not found")); }
    Ok(Json(SuccessResponse { result: "Success".to_string(), status: 200, data: serde_json::json!({}) }))
}

/// POST /v2/lists/{id} — add entry
#[utoipa::path(post, path = "/v2/lists/{id}", tag = "lists", responses(
    (status = 200, description = "Success"),
    (status = 400, description = "Bad request", body = ErrorResponseTemplate),
    (status = 401, description = "Unauthorized", body = ErrorResponseTemplate),
    (status = 404, description = "Not found", body = ErrorResponseTemplate),
    (status = 500, description = "Internal error", body = ErrorResponseTemplate),
))]
pub async fn add_entry(
    user: AuthUser, Path(list_id): Path<Uuid>, State(db): State<DbPool>,
    Json(body): Json<serde_json::Value>,
) -> Result<Json<SuccessResponse<serde_json::Value>>, ApiError> {
    let work_id: Uuid = serde_json::from_value(body["mangaId"].clone())
        .map_err(|_| ApiError::bad_request("Invalid mangaId"))?;
    let owned: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM public.user_lists WHERE id = $1 AND user_id = $2)")
        .bind(list_id).bind(&user.id).fetch_one(&db).await?;
    if !owned { return Err(ApiError::not_found("List not found")); }
    let max_order: Option<i32> = sqlx::query_scalar("SELECT MAX(order_index) FROM public.user_list_entries WHERE list_id = $1")
        .bind(list_id).fetch_one(&db).await?;
    sqlx::query("INSERT INTO public.user_list_entries (list_id, work_id, order_index) VALUES ($1, $2, $3) ON CONFLICT DO NOTHING")
        .bind(list_id).bind(work_id).bind(max_order.unwrap_or(-1) + 1).execute(&db).await?;
    Ok(Json(SuccessResponse { result: "Success".to_string(), status: 200, data: serde_json::json!({}) }))
}

/// DELETE /v2/lists/{id}/{entry_id}
#[utoipa::path(delete, path = "/v2/lists/{id}/{entryId}", tag = "lists", responses(
    (status = 200, description = "Success"),
    (status = 401, description = "Unauthorized", body = ErrorResponseTemplate),
    (status = 404, description = "Not found", body = ErrorResponseTemplate),
    (status = 500, description = "Internal error", body = ErrorResponseTemplate),
))]
pub async fn remove_entry(
    user: AuthUser, Path((list_id, work_id)): Path<(Uuid, Uuid)>, State(db): State<DbPool>,
) -> Result<Json<SuccessResponse<serde_json::Value>>, ApiError> {
    let owned: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM public.user_lists WHERE id = $1 AND user_id = $2)")
        .bind(list_id).bind(&user.id).fetch_one(&db).await?;
    if !owned { return Err(ApiError::not_found("List not found")); }
    sqlx::query("DELETE FROM public.user_list_entries WHERE list_id = $1 AND work_id = $2")
        .bind(list_id).bind(work_id).execute(&db).await?;
    Ok(Json(SuccessResponse { result: "Success".to_string(), status: 200, data: serde_json::json!({}) }))
}

/// PUT /v2/lists/{id}/{entry_id}
#[utoipa::path(put, path = "/v2/lists/{id}/{entryId}", tag = "lists", responses(
    (status = 200, description = "Success"),
    (status = 401, description = "Unauthorized", body = ErrorResponseTemplate),
    (status = 404, description = "Not found", body = ErrorResponseTemplate),
    (status = 500, description = "Internal error", body = ErrorResponseTemplate),
))]
pub async fn update_entry(
    user: AuthUser, Path((list_id, work_id)): Path<(Uuid, Uuid)>, State(db): State<DbPool>,
    Json(body): Json<UpdateEntryBody>,
) -> Result<Json<SuccessResponse<serde_json::Value>>, ApiError> {
    let owned: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM public.user_lists WHERE id = $1 AND user_id = $2)")
        .bind(list_id).bind(&user.id).fetch_one(&db).await?;
    if !owned { return Err(ApiError::not_found("List not found")); }
    if let Some(order) = body.order_index {
        sqlx::query("UPDATE public.user_list_entries SET order_index = $1 WHERE list_id = $2 AND work_id = $3")
            .bind(order).bind(list_id).bind(work_id).execute(&db).await?;
    }
    Ok(Json(SuccessResponse { result: "Success".to_string(), status: 200, data: serde_json::json!({}) }))
}

/// GET /v2/lists/user/me/manga/{manga_id}
#[utoipa::path(get, path = "/v2/lists/user/me/manga/{mangaId}", tag = "lists", responses(
    (status = 200, description = "Success", body = SuccessResponse<Vec<Uuid>>),
    (status = 401, description = "Unauthorized", body = ErrorResponseTemplate),
    (status = 500, description = "Internal error", body = ErrorResponseTemplate),
))]
pub async fn list_ids_containing_manga(
    user: AuthUser, Path(work_id): Path<Uuid>, State(db): State<DbPool>,
) -> Result<Json<SuccessResponse<serde_json::Value>>, ApiError> {
    let ids: Vec<Uuid> = sqlx::query_scalar(
        "SELECT ul.id FROM public.user_list_entries ule \
         JOIN public.user_lists ul ON ul.id = ule.list_id \
         WHERE ul.user_id = $1 AND ule.work_id = $2",
    ).bind(&user.id).bind(work_id).fetch_all(&db).await?;
    Ok(Json(SuccessResponse { result: "Success".to_string(), status: 200, data: serde_json::json!(ids) }))
}
