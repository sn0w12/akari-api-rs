use axum::Json;
use axum::extract::{Path, Query, State};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use crate::db::DbPool;
use crate::error::{ApiError, ErrorResponseTemplate};
use crate::models::work::MangaResponse;
use crate::response::{PaginatedResponse, SuccessResponse};

#[derive(Debug, Clone, Deserialize, utoipa::IntoParams)]
pub struct AuthorListParams {
    pub page: Option<i32>,
    pub page_size: Option<i32>,
}

#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct AuthorResponse {
    pub name: String,
    #[serde(rename = "mangaCount")]
    pub manga_count: i64,
}

/// GET /v2/author/{name}
#[utoipa::path(get, path = "/v2/author/{name}", tag = "author", params(crate::handlers::manga::MangaListParams), responses(
    (status = 200, description = "Success", body = SuccessResponse<PaginatedResponse<MangaResponse>>),
    (status = 500, description = "Internal error", body = ErrorResponseTemplate),
))]
pub async fn manga_by_author(
    Path(name): Path<String>,
    Query(params): Query<crate::handlers::manga::MangaListParams>,
    State(db): State<DbPool>,
) -> Result<Json<SuccessResponse<PaginatedResponse<crate::models::work::MangaResponse>>>, ApiError>
{
    let mut p = params;
    p.authors = Some(vec![name]);
    crate::handlers::manga::list_manga(axum::extract::Query(p), State(db)).await
}

/// GET /v2/author/list
#[utoipa::path(get, path = "/v2/author/list", tag = "author", params(AuthorListParams), responses(
    (status = 200, description = "Success", body = SuccessResponse<PaginatedResponse<AuthorResponse>>),
    (status = 500, description = "Internal error", body = ErrorResponseTemplate),
))]
pub async fn list_authors(
    Query(params): Query<AuthorListParams>,
    State(db): State<DbPool>,
) -> Result<Json<SuccessResponse<PaginatedResponse<AuthorResponse>>>, ApiError> {
    let page = params.page.unwrap_or(1).max(1);
    let page_size = params.page_size.unwrap_or(100).clamp(1, 500);
    let offset = ((page - 1) as i64) * (page_size as i64);

    let count_fut = sqlx::query_scalar("SELECT COUNT(*) FROM public.authors").fetch_one(&db);
    let data_fut = sqlx::query_as::<_, (String, i64)>(
        "SELECT a.name, COUNT(wa.work_id)::bigint AS manga_count \
         FROM public.authors a \
         JOIN public.work_authors wa ON wa.author_id = a.id \
         GROUP BY a.id, a.name \
         ORDER BY manga_count DESC \
         LIMIT $1 OFFSET $2",
    )
    .bind(page_size as i64)
    .bind(offset)
    .fetch_all(&db);
    let (total_count, raw_items) = tokio::try_join!(count_fut, data_fut)?;

    let total_pages = ((total_count as f64) / (page_size as f64)).ceil() as i32;

    let items: Vec<AuthorResponse> = raw_items
        .into_iter()
        .map(|(name, count)| AuthorResponse {
            name,
            manga_count: count,
        })
        .collect();

    Ok(Json(SuccessResponse {
        result: "Success".to_string(),
        status: 200,
        data: PaginatedResponse {
            items,
            total_items: total_count,
            current_page: page,
            page_size,
            total_pages,
        },
    }))
}
