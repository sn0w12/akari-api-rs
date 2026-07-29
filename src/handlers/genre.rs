use axum::extract::{Path, Query, State};
use axum::Json;
use serde::Serialize;

use crate::db::DbPool;
use crate::error::{ApiError, ErrorResponseTemplate};
use crate::handlers::manga::{list_manga, MangaListParams};
use crate::models::work::MangaResponse;
use crate::response::{PaginatedResponse, SuccessResponse};

#[derive(Debug, Serialize, utoipa::ToSchema)]
pub struct GenreResponse {
    pub id: i32,
    pub name: String,
    pub description: Option<String>,
}

/// GET /v2/genre/{name}
#[utoipa::path(get, path = "/v2/genre/{name}", tag = "genre", params(MangaListParams), responses(
    (status = 200, description = "Success", body = SuccessResponse<PaginatedResponse<MangaResponse>>),
    (status = 500, description = "Internal error", body = ErrorResponseTemplate),
))]
pub async fn manga_by_genre(
    Path(name): Path<String>,
    Query(params): Query<MangaListParams>,
    State(db): State<DbPool>,
) -> Result<Json<SuccessResponse<PaginatedResponse<crate::models::work::MangaResponse>>>, ApiError> {
    let mut p = params;
    p.genres = Some(vec![name]);
    list_manga(Query(p), State(db)).await
}

/// GET /v2/genre/list
#[utoipa::path(get, path = "/v2/genre/list", tag = "genre", responses(
    (status = 200, description = "List of genre names", body = SuccessResponse<Vec<GenreResponse>>, content_type = "application/json"),
    (status = 500, description = "Internal error", body = ErrorResponseTemplate),
))]
pub async fn list_genres(
    State(db): State<DbPool>,
) -> Result<Json<SuccessResponse<Vec<GenreResponse>>>, ApiError> {
    let rows: Vec<(String,)> =
        sqlx::query_as("SELECT DISTINCT unnest(genres) AS name FROM public.works ORDER BY name")
            .fetch_all(&db)
            .await?;
    let genres: Vec<GenreResponse> = rows
        .into_iter()
        .enumerate()
        .map(|(i, (name,))| GenreResponse {
            id: (i + 1) as i32,
            name,
            description: None,
        })
        .collect();
    Ok(Json(SuccessResponse {
        result: "Success".to_string(),
        status: 200,
        data: genres,
    }))
}
