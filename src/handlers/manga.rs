use std::collections::HashMap;

use axum::Json;
use axum::extract::{Path, Query, State};
use chrono::{DateTime, Utc};
use serde::Deserialize;
use sqlx::QueryBuilder;
use uuid::Uuid;

use crate::auth::{AuthUser, OptionalAuthUser};
use crate::db::DbPool;
use crate::error::{ApiError, ErrorResponseTemplate};
use crate::models::chapter::{
    ChapterNavigation, ChapterResponse, MangaChapter, Scanlator,
};
use crate::models::cover::Cover;
use crate::models::manga_type::WorkFormat;
use crate::models::relationship::WorkRelationship;
use crate::models::work::{
    ChapterIdsResponse, MangaChapterResponse, MangaDetailResponse, MangaIdsResponse, MangaResponse,
    MangaSearchResponse, RatingResponse,
};
use crate::response::{ItemsResponse, PaginatedResponse, SuccessResponse};

#[derive(Debug, Clone, Deserialize, utoipa::IntoParams)]
#[serde(rename_all = "camelCase")]
pub struct MangaListParams {
    pub page: Option<i32>,
    pub page_size: Option<i32>,
    pub sort_by: Option<String>,
    pub query: Option<String>,
    pub genres: Option<Vec<String>>,
    #[serde(rename = "excludedGenres")]
    pub excluded_genres: Option<Vec<String>>,
    pub authors: Option<Vec<String>>,
    pub types: Option<Vec<WorkFormat>>,
    #[serde(rename = "excludedTypes")]
    pub excluded_types: Option<Vec<WorkFormat>>,
    pub status: Option<String>,
}

#[derive(Debug, Clone, Deserialize, utoipa::IntoParams)]
#[serde(rename_all = "camelCase")]
pub struct PopularParams {
    pub page: Option<i32>,
    pub limit: Option<i32>,
    #[serde(rename = "excludedGenres")]
    pub excluded_genres: Option<Vec<String>>,
}

#[derive(Debug, Clone, Deserialize, utoipa::IntoParams)]
pub struct SearchParams {
    pub query: Option<String>,
    pub limit: Option<i32>,
}

#[derive(Debug, Clone, Deserialize, utoipa::IntoParams)]
pub struct MangaIdsParams {
    pub page: Option<i32>,
    pub page_size: Option<i32>,
}

#[derive(Debug, Clone, Deserialize, utoipa::IntoParams)]
pub struct BatchParams {
    pub ids: Option<String>,
}

#[derive(Debug, Clone, Deserialize, utoipa::IntoParams)]
pub struct ChapterDetailParams {
    #[serde(rename = "scanlatorId")]
    pub scanlation_group_id: Option<i32>,
}

fn pagination(page: Option<i32>, page_size: Option<i32>) -> (i32, i32, i64) {
    let p = page.unwrap_or(1).max(1);
    let ps = page_size.unwrap_or(20).clamp(1, 100);
    (p, ps, ((p - 1) as i64) * (ps as i64))
}

#[derive(Debug, sqlx::FromRow)]
#[allow(dead_code)]
struct MangaListRow {
    id: Uuid,
    title: String,
    description: String,
    status: String,
    #[sqlx(rename = "format")]
    manga_type: String,
    genres: Vec<String>,
    alternative_titles: String,
    view_count: i64,
    score: Option<f64>,
    trackers: String,
    preferred_scanlation_group_id: Option<i32>,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
    cover_url: Option<String>,
    cover_thumbhash: Option<String>,
    authors: Vec<String>,
    rating_count: Option<i64>,
    average_rating: Option<f64>,
}

impl From<MangaListRow> for MangaResponse {
    fn from(r: MangaListRow) -> Self {
        Self {
            id: r.id,
            title: r.title,
            cover: Cover {
                url: r.cover_url.unwrap_or_default(),
                thumbhash: r.cover_thumbhash,
            },
            description: r.description,
            status: r.status,
            manga_type: WorkFormat::from(&r.manga_type as &str),
            authors: r.authors,
            genres: r.genres,
            views: r.view_count as i32,
            rating: crate::models::work::MangaRatingResponse {
                average: r.score.unwrap_or(0.0),
                total: r.rating_count.unwrap_or(0) as i32,
                distribution: crate::models::work::MangaRatingDistribution {
                    score1: 0,
                    score2: 0,
                    score3: 0,
                    score4: 0,
                    score5: 0,
                    score6: 0,
                    score7: 0,
                    score8: 0,
                    score9: 0,
                    score10: 0,
                },
            },
            alternative_titles: serde_json::from_str(&r.alternative_titles).unwrap_or_default(),
            trackers: serde_json::from_str(&r.trackers).unwrap_or_default(),
            preferred_scanlator_id: r.preferred_scanlation_group_id,
            created_at: r.created_at,
            updated_at: r.updated_at,
        }
    }
}

#[derive(Debug, sqlx::FromRow)]
#[allow(dead_code)]
struct ChapterRow {
    id: Uuid,
    work_id: Uuid,
    number: f64,
    title: Option<String>,
    language_code: String,
    pages: Option<i16>,
    images: Vec<String>,
    scanlation_group_id: Option<i32>,
    scanlation_group_name: Option<String>,
    released_at: Option<DateTime<Utc>>,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

#[derive(Debug, sqlx::FromRow)]
struct ChapterListRow {
    id: Uuid,
    number: f64,
    title: Option<String>,
    scanlation_group_id: Option<i32>,
    pages: Option<i16>,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

impl From<ChapterRow> for MangaChapter {
    fn from(r: ChapterRow) -> Self {
        Self {
            id: r.id,
            title: r
                .title
                .clone()
                .unwrap_or_else(|| format!("Chapter {}", r.number)),
            number: r.number,
            scanlator_id: r.scanlation_group_id.unwrap_or(0),
            pages: r.pages.map(|p| p as i32),
            created_at: r.created_at,
            updated_at: r.updated_at,
        }
    }
}

macro_rules! manga_full_sql {
    ($suffix:literal) => {
        concat!(
            "SELECT w.id, w.title, w.description, w.status, w.format, w.genres, \
             w.view_count, w.score::double precision AS score, w.preferred_scanlation_group_id, \
             w.created_at, w.updated_at, \
             cov.url AS cover_url, cov.thumbhash AS cover_thumbhash, auth.authors, alt.alternative_titles, \
             tr.trackers, r.rating_count, r.average_rating \
             FROM public.works w \
             LEFT JOIN LATERAL (SELECT url, thumbhash FROM public.covers WHERE work_id = w.id AND is_preferred = TRUE LIMIT 1) cov ON TRUE \
             LEFT JOIN LATERAL (SELECT COALESCE(ARRAY_AGG(a.name ORDER BY wa.position, a.name), '{}'::text[]) AS authors \
               FROM public.work_authors wa JOIN public.authors a ON a.id = wa.author_id WHERE wa.work_id = w.id) auth ON TRUE \
             LEFT JOIN LATERAL (SELECT COALESCE(json_agg(json_build_object('title', wt.title, 'languageCode', wt.language_code, 'titleType', wt.title_type) ORDER BY wt.language_code, wt.title_type), '[]'::json)::text AS alternative_titles \
               FROM public.work_titles wt WHERE wt.work_id = w.id) alt ON TRUE \
             LEFT JOIN LATERAL (SELECT COALESCE(json_agg(json_build_object('code', t.code, 'id', wt.tracker_work_id) ORDER BY t.code), '[]'::json)::text AS trackers \
               FROM public.work_trackers wt JOIN public.trackers t ON t.id = wt.tracker_id WHERE wt.work_id = w.id) tr ON TRUE \
             LEFT JOIN LATERAL (SELECT COUNT(*)::bigint AS rating_count, COALESCE(AVG(rating), 0)::double precision AS average_rating \
               FROM public.work_ratings WHERE work_id = w.id) r ON TRUE ",
            $suffix,
        )
    };
}

const MANGA_BY_ID_SQL: &str = manga_full_sql!("WHERE w.id = $1");
const MANGA_BATCH_SQL: &str = manga_full_sql!("WHERE w.id = ANY($1)");

/// Static SQL for the list/popular QueryBuilder data queries.
const MANGA_LIST_FROM: &str = "\
FROM public.works w \
LEFT JOIN LATERAL (SELECT url, thumbhash FROM public.covers WHERE work_id = w.id AND is_preferred = TRUE LIMIT 1) cov ON TRUE \
LEFT JOIN LATERAL (SELECT COALESCE(ARRAY_AGG(a.name ORDER BY wa.position, a.name), '{}'::text[]) AS authors \
  FROM public.work_authors wa JOIN public.authors a ON a.id = wa.author_id WHERE wa.work_id = w.id) auth ON TRUE \
LEFT JOIN LATERAL (SELECT COALESCE(json_agg(json_build_object('title', wt.title, 'languageCode', wt.language_code, 'titleType', wt.title_type) ORDER BY wt.language_code, wt.title_type), '[]'::json)::text AS alternative_titles \
  FROM public.work_titles wt WHERE wt.work_id = w.id) alt ON TRUE \
LEFT JOIN LATERAL (SELECT COALESCE(json_agg(json_build_object('code', t.code, 'id', wt.tracker_work_id) ORDER BY t.code), '[]'::json)::text AS trackers \
  FROM public.work_trackers wt JOIN public.trackers t ON t.id = wt.tracker_id WHERE wt.work_id = w.id) tr ON TRUE \
LEFT JOIN LATERAL (SELECT COUNT(*)::bigint AS rating_count, COALESCE(AVG(rating), 0)::double precision AS average_rating \
  FROM public.work_ratings WHERE work_id = w.id) r ON TRUE";

struct MangaFilters {
    genres: Vec<String>,
    excluded_genres: Vec<String>,
    authors: Vec<String>,
    types: Vec<String>,
    excluded_types: Vec<String>,
    status: Option<String>,
    search_query: Option<String>,
}

fn apply_filters(builder: &mut QueryBuilder<sqlx::Postgres>, f: &MangaFilters) {
    if !f.genres.is_empty() {
        builder.push(" AND w.genres && ");
        builder.push_bind(&f.genres);
    }
    if !f.excluded_genres.is_empty() {
        builder.push(" AND NOT (w.genres && ");
        builder.push_bind(&f.excluded_genres);
        builder.push(")");
    }
    if !f.authors.is_empty() {
        builder.push(" AND EXISTS (SELECT 1 FROM public.work_authors wa JOIN public.authors a ON a.id = wa.author_id WHERE wa.work_id = w.id AND a.name = ANY(");
        builder.push_bind(&f.authors);
        builder.push("))");
    }
    if !f.types.is_empty() {
        builder.push(" AND w.format = ANY(");
        builder.push_bind(&f.types);
        builder.push(")");
    }
    if !f.excluded_types.is_empty() {
        builder.push(" AND NOT (w.format = ANY(");
        builder.push_bind(&f.excluded_types);
        builder.push("))");
    }
    if let Some(ref status) = f.status {
        builder.push(" AND w.status = ");
        builder.push_bind(status);
    }
    if let Some(ref q) = f.search_query {
        builder.push(" AND w.search_vector @@ plainto_tsquery('english', ");
        builder.push_bind(q);
        builder.push(")");
    }
}

/// GET /v2/manga/list
#[utoipa::path(get, path = "/v2/manga/list", tag = "manga", params(MangaListParams), responses(
    (status = 200, description = "Success", body = SuccessResponse<PaginatedResponse<MangaResponse>>),
    (status = 500, description = "Internal error", body = ErrorResponseTemplate),
))]
pub async fn list_manga(
    Query(params): Query<MangaListParams>,
    State(db): State<DbPool>,
) -> Result<Json<SuccessResponse<PaginatedResponse<MangaResponse>>>, ApiError> {
    let (page, page_size, offset) = pagination(params.page, params.page_size);

    let sort_by = match params.sort_by.as_deref() {
        Some("popular") => "popular",
        Some("newest") => "newest",
        Some("search") => "search",
        _ => "latest",
    };

    let filters = MangaFilters {
        genres: params.genres.clone().unwrap_or_default(),
        excluded_genres: params.excluded_genres.clone().unwrap_or_default(),
        authors: params.authors.clone().unwrap_or_default(),
        types: params
            .types
            .clone()
            .map(|v| v.into_iter().map(|t| t.as_str().to_string()).collect())
            .unwrap_or_default(),
        excluded_types: params
            .excluded_types
            .clone()
            .map(|v| v.into_iter().map(|t| t.as_str().to_string()).collect())
            .unwrap_or_default(),
        status: params.status.filter(|s| !s.is_empty()),
        search_query: if sort_by == "search" {
            params.query.filter(|s| !s.is_empty())
        } else {
            None
        },
    };

    // Count and page query are independent; build both before awaiting either.
    let mut count_builder = QueryBuilder::new("SELECT COUNT(*) FROM public.works w WHERE 1=1");
    apply_filters(&mut count_builder, &filters);

    // Data
    let mut data_builder = QueryBuilder::new(
        "SELECT w.id, w.title, w.description, w.status, w.format, w.genres, \
         w.view_count, w.score::double precision AS score, w.preferred_scanlation_group_id, \
         w.created_at, w.updated_at, \
         cov.url AS cover_url, cov.thumbhash AS cover_thumbhash, auth.authors, alt.alternative_titles, \
         tr.trackers, r.rating_count, r.average_rating ",
    );
    data_builder.push(MANGA_LIST_FROM);
    data_builder.push(" WHERE 1=1");
    apply_filters(&mut data_builder, &filters);

    match sort_by {
        "popular" => {
            data_builder.push(" ORDER BY w.view_count DESC");
        }
        "newest" => {
            data_builder.push(" ORDER BY w.created_at DESC");
        }
        "search" => {
            if let Some(q) = &filters.search_query {
                data_builder.push(" ORDER BY CASE WHEN lower(trim(w.title)) = lower(trim(");
                data_builder.push_bind(q);
                data_builder.push(")) THEN 0 WHEN position(lower(trim(");
                data_builder.push_bind(q);
                data_builder.push(
                    ")) in lower(trim(w.title))) = 1 THEN 1 ELSE 2 END ASC, \
                     ts_rank(w.search_vector, phraseto_tsquery('english', ",
                );
                data_builder.push_bind(q);
                data_builder.push(
                    ")) DESC NULLS LAST, ts_rank(w.search_vector, plainto_tsquery('english', ",
                );
                data_builder.push_bind(q);
                data_builder.push(")) DESC, CASE WHEN position(lower(trim(");
                data_builder.push_bind(q);
                data_builder.push(")) in lower(trim(w.title))) > 0 THEN position(lower(trim(");
                data_builder.push_bind(q);
                data_builder.push(
                    ")) in lower(trim(w.title))) ELSE 2147483647 END ASC, \
                     w.score DESC NULLS LAST, w.view_count DESC, w.updated_at DESC, w.id ASC",
                );
            } else {
                data_builder.push(" ORDER BY w.updated_at DESC");
            }
        }
        _ => {
            data_builder.push(" ORDER BY w.updated_at DESC");
        }
    }

    data_builder.push(" LIMIT ");
    data_builder.push_bind(page_size as i64);
    data_builder.push(" OFFSET ");
    data_builder.push_bind(offset);

    let count_fut = count_builder.build_query_scalar().fetch_one(&db);
    let data_fut = data_builder.build_query_as().fetch_all(&db);
    let (total_count, rows): (i64, Vec<MangaListRow>) = tokio::try_join!(count_fut, data_fut)?;
    let total_pages = ((total_count as f64) / (page_size as f64)).ceil() as i32;
    let items: Vec<MangaResponse> = rows.into_iter().map(Into::into).collect();

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

/// GET /v2/manga/popular
#[utoipa::path(get, path = "/v2/manga/list/popular", tag = "manga", params(PopularParams), responses(
    (status = 200, description = "Success", body = SuccessResponse<PaginatedResponse<MangaResponse>>),
    (status = 500, description = "Internal error", body = ErrorResponseTemplate),
))]
pub async fn popular_manga(
    Query(params): Query<PopularParams>,
    State(db): State<DbPool>,
) -> Result<Json<SuccessResponse<PaginatedResponse<MangaResponse>>>, ApiError> {
    let page = params.page.unwrap_or(1).max(1);
    let page_size = params.limit.unwrap_or(10).clamp(1, 50);
    let offset = ((page - 1) as i64) * (page_size as i64);
    let excluded_genres = params.excluded_genres.unwrap_or_default();

    // Count and page query are independent; build both before awaiting either.
    let mut count_builder = QueryBuilder::new("SELECT COUNT(*) FROM public.works w WHERE 1=1");
    if !excluded_genres.is_empty() {
        count_builder.push(" AND NOT (w.genres && ");
        count_builder.push_bind(&excluded_genres);
        count_builder.push(")");
    }

    // Data
    let mut data_builder = QueryBuilder::new(
        "SELECT w.id, w.title, w.description, w.status, w.format, w.genres, \
         w.view_count, w.score::double precision AS score, w.preferred_scanlation_group_id, \
         w.created_at, w.updated_at, \
         cov.url AS cover_url, cov.thumbhash AS cover_thumbhash, auth.authors, alt.alternative_titles, \
         tr.trackers, r.rating_count, r.average_rating ",
    );
    data_builder.push(MANGA_LIST_FROM);
    data_builder.push(" WHERE 1=1");

    if !excluded_genres.is_empty() {
        data_builder.push(" AND NOT (w.genres && ");
        data_builder.push_bind(&excluded_genres);
        data_builder.push(")");
    }

    data_builder.push(" ORDER BY w.view_count DESC LIMIT ");
    data_builder.push_bind(page_size as i64);
    data_builder.push(" OFFSET ");
    data_builder.push_bind(offset);

    let count_fut = count_builder.build_query_scalar().fetch_one(&db);
    let data_fut = data_builder.build_query_as().fetch_all(&db);
    let (total_count, rows): (i64, Vec<MangaListRow>) = tokio::try_join!(count_fut, data_fut)?;
    let total_pages = ((total_count as f64) / (page_size as f64)).ceil() as i32;
    let items: Vec<MangaResponse> = rows.into_iter().map(Into::into).collect();

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

/// GET /v2/manga/{id}
#[utoipa::path(get, path = "/v2/manga/{id}", tag = "manga", responses(
    (status = 200, description = "Success", body = SuccessResponse<MangaResponse>),
    (status = 404, description = "Not found", body = ErrorResponseTemplate),
    (status = 500, description = "Internal error", body = ErrorResponseTemplate),
))]
pub async fn get_manga(
    Path(id): Path<Uuid>,
    State(db): State<DbPool>,
) -> Result<Json<SuccessResponse<MangaResponse>>, ApiError> {
    let manga_row: MangaListRow = sqlx::query_as::<_, MangaListRow>(MANGA_BY_ID_SQL)
        .bind(id)
        .fetch_optional(&db)
        .await?
        .ok_or(ApiError::not_found("Manga not found"))?;
    Ok(Json(SuccessResponse {
        result: "Success".to_string(),
        status: 200,
        data: manga_row.into(),
    }))
}

/// GET /v2/manga/{id}/details
#[utoipa::path(get, path = "/v2/manga/{id}/details", tag = "manga", responses(
    (status = 200, description = "Success", body = SuccessResponse<MangaDetailResponse>),
    (status = 404, description = "Not found", body = ErrorResponseTemplate),
    (status = 500, description = "Internal error", body = ErrorResponseTemplate),
))]
pub async fn manga_details(
    Path(id): Path<Uuid>,
    State(db): State<DbPool>,
) -> Result<Json<SuccessResponse<MangaDetailResponse>>, ApiError> {
    // Manga row and chapters are independent; run them concurrently. The 404
    // branch is preserved: an absent manga row wins over the chapters result.
    let manga_fut = async {
        sqlx::query_as::<_, MangaListRow>(MANGA_BY_ID_SQL)
            .bind(id)
            .fetch_optional(&db)
            .await
            .map_err(ApiError::from)
    };
    let chapters_fut = fetch_chapters_for_work(&db, id);
    let (manga_row, chapters) = tokio::try_join!(manga_fut, chapters_fut)?;
    let manga_row = manga_row.ok_or(ApiError::not_found("Manga not found"))?;
    let m: MangaResponse = manga_row.into();
    Ok(Json(SuccessResponse {
        result: "Success".to_string(),
        status: 200,
        data: MangaDetailResponse {
            id: m.id,
            title: m.title,
            cover: m.cover,
            description: m.description,
            status: m.status,
            manga_type: m.manga_type,
            authors: m.authors,
            genres: m.genres,
            views: m.views,
            rating: m.rating,
            alternative_titles: m.alternative_titles,
            trackers: m.trackers.clone(),
            preferred_scanlator_id: m.preferred_scanlator_id,
            created_at: m.created_at,
            updated_at: m.updated_at,
            chapters,
        },
    }))
}

/// GET /v2/manga/{id}/chapters
#[utoipa::path(get, path = "/v2/manga/{id}/chapters", tag = "manga", responses(
    (status = 200, description = "Success", body = SuccessResponse<MangaChapterResponse>),
    (status = 500, description = "Internal error", body = ErrorResponseTemplate),
))]
pub async fn manga_chapters(
    Path(id): Path<Uuid>,
    State(db): State<DbPool>,
) -> Result<Json<SuccessResponse<MangaChapterResponse>>, ApiError> {
    let chapters_fut = fetch_chapters_for_work(&db, id);
    let preferred_fut = async {
        sqlx::query_scalar("SELECT preferred_scanlation_group_id FROM public.works WHERE id = $1")
            .bind(id)
            .fetch_optional(&db)
            .await
            .map_err(ApiError::from)
    };
    let scanlators_fut = async {
        sqlx::query_as::<_, (i32, String)>(
            "SELECT DISTINCT sg.id, sg.name FROM public.scanlation_groups sg \
             JOIN public.chapters c ON c.scanlation_group_id = sg.id \
             WHERE c.work_id = $1 AND sg.id IS NOT NULL \
             ORDER BY sg.name",
        )
        .bind(id)
        .fetch_all(&db)
        .await
        .map_err(ApiError::from)
    };

    let (chapters, preferred_scanlation_group_id, scanlators) = tokio::try_join!(
        chapters_fut,
        preferred_fut,
        scanlators_fut,
    )?;
    let preferred_scanlation_group_id = preferred_scanlation_group_id.flatten();

    let scanlators = scanlators
        .into_iter()
        .map(|(id, name)| Scanlator { id, name })
        .collect();

    Ok(Json(SuccessResponse {
        result: "Success".to_string(),
        status: 200,
        data: MangaChapterResponse {
            scanlators,
            chapters,
            preferred_scanlator_id: preferred_scanlation_group_id,
        },
    }))
}

#[derive(Debug, Deserialize, utoipa::ToSchema, utoipa::IntoParams)]
pub struct BatchMalBody {
    #[serde(rename = "malIds")]
    pub mal_ids: Vec<i32>,
}

#[derive(Debug, Deserialize, utoipa::ToSchema, utoipa::IntoParams)]
pub struct BatchAniBody {
    #[serde(rename = "aniIds")]
    pub ani_ids: Vec<i32>,
}

async fn batch_by_tracker(
    db: &DbPool,
    tracker_code: &str,
    ids: &[String],
) -> Result<Vec<MangaResponse>, ApiError> {
    let mut builder = QueryBuilder::new(
        "SELECT w.id, w.title, w.description, w.status, w.format, w.genres, \
         w.view_count, w.score::double precision AS score, w.preferred_scanlation_group_id, \
         w.created_at, w.updated_at, \
         cov.url AS cover_url, cov.thumbhash AS cover_thumbhash, auth.authors, alt.alternative_titles, \
         tr.trackers, r.rating_count, r.average_rating \
          FROM public.works w \
          LEFT JOIN LATERAL (SELECT url, thumbhash FROM public.covers WHERE work_id = w.id AND is_preferred = TRUE LIMIT 1) cov ON TRUE \
          LEFT JOIN LATERAL (SELECT COALESCE(ARRAY_AGG(a.name ORDER BY wa.position, a.name), '{}'::text[]) AS authors \
            FROM public.work_authors wa JOIN public.authors a ON a.id = wa.author_id WHERE wa.work_id = w.id) auth ON TRUE \
          LEFT JOIN LATERAL (SELECT COALESCE(json_agg(json_build_object('title', wt.title, 'languageCode', wt.language_code, 'titleType', wt.title_type) ORDER BY wt.language_code, wt.title_type), '[]'::json)::text AS alternative_titles \
            FROM public.work_titles wt WHERE wt.work_id = w.id) alt ON TRUE \
          LEFT JOIN LATERAL (SELECT COALESCE(json_agg(json_build_object('code', t.code, 'id', wt.tracker_work_id) ORDER BY t.code), '[]'::json)::text AS trackers \
            FROM public.work_trackers wt JOIN public.trackers t ON t.id = wt.tracker_id WHERE wt.work_id = w.id) tr ON TRUE \
          LEFT JOIN LATERAL (SELECT COUNT(*)::bigint AS rating_count, COALESCE(AVG(rating), 0)::double precision AS average_rating \
            FROM public.work_ratings WHERE work_id = w.id) r ON TRUE \
          WHERE w.id IN (SELECT wt2.work_id FROM public.work_trackers wt2 \
            JOIN public.trackers t2 ON t2.id = wt2.tracker_id WHERE t2.code = ",
    );
    builder.push_bind(tracker_code);
    builder.push(" AND wt2.tracker_work_id = ANY(");
    builder.push_bind(ids.to_vec());
    builder.push("))");

    let rows: Vec<MangaListRow> = builder.build_query_as().fetch_all(db).await?;
    Ok(rows.into_iter().map(Into::into).collect())
}

/// POST /v2/manga/mal/batch
#[utoipa::path(post, path = "/v2/manga/mal/batch", tag = "manga", responses(
    (status = 200, description = "Success", body = SuccessResponse<Vec<MangaResponse>>),
    (status = 400, description = "Bad request", body = ErrorResponseTemplate),
    (status = 500, description = "Internal error", body = ErrorResponseTemplate),
))]
pub async fn batch_by_mal(
    State(db): State<DbPool>,
    Json(body): Json<BatchMalBody>,
) -> Result<Json<SuccessResponse<Vec<MangaResponse>>>, ApiError> {
    if body.mal_ids.len() > 50 {
        return Err(ApiError::bad_request("Max 50 IDs per batch"));
    }
    let ids: Vec<String> = body.mal_ids.iter().map(|i| i.to_string()).collect();
    let items = batch_by_tracker(&db, "myanimelist", &ids).await?;
    Ok(Json(SuccessResponse {
        result: "Success".to_string(),
        status: 200,
        data: items,
    }))
}

/// POST /v2/manga/ani/batch
#[utoipa::path(post, path = "/v2/manga/ani/batch", tag = "manga", responses(
    (status = 200, description = "Success", body = SuccessResponse<Vec<MangaResponse>>),
    (status = 400, description = "Bad request", body = ErrorResponseTemplate),
    (status = 500, description = "Internal error", body = ErrorResponseTemplate),
))]
pub async fn batch_by_ani(
    State(db): State<DbPool>,
    Json(body): Json<BatchAniBody>,
) -> Result<Json<SuccessResponse<Vec<MangaResponse>>>, ApiError> {
    if body.ani_ids.len() > 50 {
        return Err(ApiError::bad_request("Max 50 IDs per batch"));
    }
    let ids: Vec<String> = body.ani_ids.iter().map(|i| i.to_string()).collect();
    let items = batch_by_tracker(&db, "anilist", &ids).await?;
    Ok(Json(SuccessResponse {
        result: "Success".to_string(),
        status: 200,
        data: items,
    }))
}

/// GET /v2/manga/{id}/recommendations
#[utoipa::path(get, path = "/v2/manga/{id}/recommendations", tag = "manga", params(PopularParams), responses(
    (status = 200, description = "Success", body = SuccessResponse<Vec<MangaResponse>>),
    (status = 500, description = "Internal error", body = ErrorResponseTemplate),
))]
pub async fn manga_recommendations(
    Path(id): Path<Uuid>,
    Query(params): Query<PopularParams>,
    State(db): State<DbPool>,
) -> Result<Json<SuccessResponse<Vec<MangaResponse>>>, ApiError> {
    let limit = params.limit.unwrap_or(20).clamp(1, 100);

    // Select the random candidate set first (materialized, limited), then run
    // the full projection only for the selected IDs. Preserves ORDER BY
    // RANDOM() set and order semantics via a window row number.
    let sql = "WITH candidates AS MATERIALIZED ( \
         SELECT id, row_number() OVER (ORDER BY random()) AS rn \
         FROM (SELECT id FROM public.works \
               WHERE id != $1 AND genres && (SELECT genres FROM public.works WHERE id = $1)) s \
         LIMIT $2 \
       ) \
       SELECT w.id, w.title, w.description, w.status, w.format, w.genres, \
         w.view_count, w.score::double precision AS score, w.preferred_scanlation_group_id, \
         w.created_at, w.updated_at, \
         cov.url AS cover_url, cov.thumbhash AS cover_thumbhash, auth.authors, alt.alternative_titles, \
         tr.trackers, r.rating_count, r.average_rating \
       FROM candidates c \
       JOIN public.works w ON w.id = c.id \
       LEFT JOIN LATERAL (SELECT url, thumbhash FROM public.covers WHERE work_id = w.id AND is_preferred = TRUE LIMIT 1) cov ON TRUE \
       LEFT JOIN LATERAL (SELECT COALESCE(ARRAY_AGG(a.name ORDER BY wa.position, a.name), '{}'::text[]) AS authors \
         FROM public.work_authors wa JOIN public.authors a ON a.id = wa.author_id WHERE wa.work_id = w.id) auth ON TRUE \
       LEFT JOIN LATERAL (SELECT COALESCE(json_agg(json_build_object('title', wt.title, 'languageCode', wt.language_code, 'titleType', wt.title_type) ORDER BY wt.language_code, wt.title_type), '[]'::json)::text AS alternative_titles \
         FROM public.work_titles wt WHERE wt.work_id = w.id) alt ON TRUE \
       LEFT JOIN LATERAL (SELECT COALESCE(json_agg(json_build_object('code', t.code, 'id', wt.tracker_work_id) ORDER BY t.code), '[]'::json)::text AS trackers \
         FROM public.work_trackers wt JOIN public.trackers t ON t.id = wt.tracker_id WHERE wt.work_id = w.id) tr ON TRUE \
       LEFT JOIN LATERAL (SELECT COUNT(*)::bigint AS rating_count, COALESCE(AVG(rating), 0)::double precision AS average_rating \
         FROM public.work_ratings WHERE work_id = w.id) r ON TRUE \
       ORDER BY c.rn";

    let rows: Vec<MangaListRow> = sqlx::query_as::<_, MangaListRow>(sql)
        .bind(id)
        .bind(limit as i64)
        .fetch_all(&db)
        .await?;

    let items: Vec<MangaResponse> = rows.into_iter().map(Into::into).collect();

    Ok(Json(SuccessResponse {
        result: "Success".to_string(),
        status: 200,
        data: items,
    }))
}

#[derive(Debug, sqlx::FromRow)]
struct RelationshipEdge {
    related_work_id: Uuid,
    relationship_type: String,
}

const WORK_RELATIONSHIPS_SQL: &str = "\
SELECT related_work_id, relationship_type FROM public.work_relationships WHERE work_id = $1 \
UNION \
SELECT work_id AS related_work_id, \
       CASE relationship_type \
         WHEN 'prequel' THEN 'sequel' \
         WHEN 'sequel' THEN 'prequel' \
         ELSE relationship_type \
       END AS relationship_type \
FROM public.work_relationships WHERE related_work_id = $1 \
ORDER BY relationship_type";

/// GET /v2/manga/{id}/relationships
#[utoipa::path(get, path = "/v2/manga/{id}/relationships", tag = "manga", responses(
    (status = 200, description = "Success", body = SuccessResponse<Vec<WorkRelationship>>),
    (status = 404, description = "Not found", body = ErrorResponseTemplate),
    (status = 500, description = "Internal error", body = ErrorResponseTemplate),
))]
pub async fn get_work_relationships(
    Path(id): Path<Uuid>,
    State(db): State<DbPool>,
) -> Result<Json<SuccessResponse<Vec<WorkRelationship>>>, ApiError> {
    let exists: bool =
        sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM public.works WHERE id = $1)")
            .bind(id)
            .fetch_one(&db)
            .await?;
    if !exists {
        return Err(ApiError::not_found("Manga not found"));
    }

    let edges: Vec<RelationshipEdge> = sqlx::query_as::<_, RelationshipEdge>(WORK_RELATIONSHIPS_SQL)
        .bind(id)
        .fetch_all(&db)
        .await?;

    if edges.is_empty() {
        return Ok(Json(SuccessResponse {
            result: "Success".to_string(),
            status: 200,
            data: vec![],
        }));
    }

    let related_ids: Vec<Uuid> = edges.iter().map(|e| e.related_work_id).collect();

    let rows: Vec<MangaListRow> = sqlx::query_as::<_, MangaListRow>(MANGA_BATCH_SQL)
        .bind(&related_ids)
        .fetch_all(&db)
        .await?;

    let mut rows_by_id: HashMap<Uuid, MangaListRow> =
        rows.into_iter().map(|r| (r.id, r)).collect();

    let items: Vec<WorkRelationship> = edges
        .into_iter()
        .filter_map(|e| {
            rows_by_id.remove(&e.related_work_id).map(|row| {
                let manga: MangaResponse = row.into();
                WorkRelationship {
                    relationship_type: e.relationship_type.into(),
                    manga,
                }
            })
        })
        .collect();

    Ok(Json(SuccessResponse {
        result: "Success".to_string(),
        status: 200,
        data: items,
    }))
}

/// GET /v2/manga/search
#[utoipa::path(get, path = "/v2/manga/search", tag = "manga", params(SearchParams), responses(
    (status = 200, description = "Success", body = SuccessResponse<Vec<MangaSearchResponse>>),
    (status = 500, description = "Internal error", body = ErrorResponseTemplate),
))]
pub async fn search_manga(
    Query(params): Query<SearchParams>,
    State(db): State<DbPool>,
) -> Result<Json<SuccessResponse<Vec<MangaSearchResponse>>>, ApiError> {
    let limit = params.limit.unwrap_or(20).clamp(1, 100);
    let query = params
        .query
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty());

    let items: Vec<MangaSearchResponse> = if let Some(ref q) = query {
        const SQL: &str = "SELECT w.id, w.title, w.description, w.status, w.format, w.genres, w.view_count, w.score::double precision AS score, \
             w.created_at, w.updated_at, cov.url AS cover_url, cov.thumbhash AS cover_thumbhash, auth.authors, \
             alt.alternative_titles, tr.trackers, \
             ts_rank(w.search_vector, plainto_tsquery('english', $1))::double precision AS rank \
             FROM public.works w \
             LEFT JOIN LATERAL (SELECT url, thumbhash FROM public.covers WHERE work_id = w.id AND is_preferred = TRUE LIMIT 1) cov ON TRUE \
             LEFT JOIN LATERAL (SELECT COALESCE(ARRAY_AGG(a.name ORDER BY wa.position, a.name), '{}'::text[]) AS authors \
               FROM public.work_authors wa JOIN public.authors a ON a.id = wa.author_id WHERE wa.work_id = w.id) auth ON TRUE \
             LEFT JOIN LATERAL (SELECT COALESCE(json_agg(json_build_object('title', wt.title, 'languageCode', wt.language_code, 'titleType', wt.title_type) ORDER BY wt.language_code, wt.title_type), '[]'::json)::text AS alternative_titles \
               FROM public.work_titles wt WHERE wt.work_id = w.id) alt ON TRUE \
             LEFT JOIN LATERAL (SELECT COALESCE(json_agg(json_build_object('code', t.code, 'id', wt.tracker_work_id) ORDER BY t.code), '[]'::json)::text AS trackers \
               FROM public.work_trackers wt JOIN public.trackers t ON t.id = wt.tracker_id WHERE wt.work_id = w.id) tr ON TRUE \
             WHERE w.search_vector @@ plainto_tsquery('english', $1) \
             ORDER BY CASE WHEN lower(trim(w.title)) = lower(trim($1)) THEN 0 \
                           WHEN position(lower(trim($1)) in lower(trim(w.title))) = 1 THEN 1 \
                           ELSE 2 END ASC, \
                      ts_rank(w.search_vector, phraseto_tsquery('english', $1)) DESC NULLS LAST, \
                      rank DESC, \
                      CASE WHEN position(lower(trim($1)) in lower(trim(w.title))) > 0 \
                           THEN position(lower(trim($1)) in lower(trim(w.title))) \
                           ELSE 2147483647 END ASC, \
                      w.score DESC NULLS LAST, w.view_count DESC, w.updated_at DESC, w.id ASC \
             LIMIT $2";

        #[derive(Debug, sqlx::FromRow)]
        struct MangaSearchRow {
            id: Uuid,
            title: String,
            description: String,
            status: String,
            #[sqlx(rename = "format")]
            manga_type: String,
            genres: Vec<String>,
            view_count: i64,
            score: Option<f64>,
            created_at: DateTime<Utc>,
            updated_at: DateTime<Utc>,
            cover_url: Option<String>,
            cover_thumbhash: Option<String>,
            authors: Vec<String>,
            alternative_titles: String,
            trackers: String,
            rank: f64,
        }

        let rows: Vec<MangaSearchRow> = sqlx::query_as::<_, MangaSearchRow>(SQL)
            .bind(q)
            .bind(limit as i64)
            .fetch_all(&db)
            .await?;

        rows.into_iter()
            .map(|r| MangaSearchResponse {
                id: r.id,
                title: r.title,
                cover: Cover {
                    url: r.cover_url.unwrap_or_default(),
                    thumbhash: r.cover_thumbhash,
                },
                description: r.description,
                status: r.status,
                manga_type: WorkFormat::from(&r.manga_type as &str),
                authors: r.authors,
                genres: r.genres,
                views: r.view_count as i32,
                rating: crate::models::work::MangaRatingResponse {
                    average: r.score.unwrap_or(0.0),
                    total: 0,
                    distribution: crate::models::work::MangaRatingDistribution {
                        score1: 0,
                        score2: 0,
                        score3: 0,
                        score4: 0,
                        score5: 0,
                        score6: 0,
                        score7: 0,
                        score8: 0,
                        score9: 0,
                        score10: 0,
                    },
                },
                alternative_titles: serde_json::from_str(&r.alternative_titles).unwrap_or_default(),
                trackers: serde_json::from_str(&r.trackers).unwrap_or_default(),
                preferred_scanlator_id: None,
                created_at: r.created_at,
                updated_at: r.updated_at,
                rank: r.rank,
            })
            .collect()
    } else {
        Vec::new()
    };

    Ok(Json(SuccessResponse {
        result: "Success".to_string(),
        status: 200,
        data: items,
    }))
}

/// GET /v2/manga/ids
#[utoipa::path(get, path = "/v2/manga/ids", tag = "manga", params(MangaIdsParams), responses(
    (status = 200, description = "Success", body = SuccessResponse<MangaIdsResponse>),
    (status = 500, description = "Internal error", body = ErrorResponseTemplate),
))]
pub async fn manga_ids(
    Query(params): Query<MangaIdsParams>,
    State(db): State<DbPool>,
) -> Result<Json<SuccessResponse<MangaIdsResponse>>, ApiError> {
    let (page, page_size, offset) = pagination(params.page, params.page_size);

    let total_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM public.works")
        .fetch_one(&db)
        .await?;

    let total_pages = ((total_count as f64) / (page_size as f64)).ceil() as i32;

    let ids: Vec<Uuid> = sqlx::query_scalar(
        "SELECT id FROM public.works ORDER BY updated_at DESC LIMIT $1 OFFSET $2",
    )
    .bind(page_size as i64)
    .bind(offset)
    .fetch_all(&db)
    .await?;

    Ok(Json(SuccessResponse {
        result: "Success".to_string(),
        status: 200,
        data: MangaIdsResponse {
            items: ids,
            total_items: total_count,
            current_page: page,
            page_size,
            total_pages,
        },
    }))
}

/// GET /v2/manga/chapter/ids (global, paginated)
#[utoipa::path(get, path = "/v2/manga/chapter/ids", tag = "manga", params(MangaIdsParams), responses(
    (status = 200, description = "Success", body = SuccessResponse<MangaIdsResponse>),
    (status = 500, description = "Internal error", body = ErrorResponseTemplate),
))]
pub async fn global_chapter_ids(
    Query(params): Query<MangaIdsParams>,
    State(db): State<DbPool>,
) -> Result<Json<SuccessResponse<MangaIdsResponse>>, ApiError> {
    let (page, page_size, offset) = pagination(params.page, params.page_size);
    let total_count: i64 =
        sqlx::query_scalar("SELECT COUNT(DISTINCT work_id) FROM public.chapters")
            .fetch_one(&db)
            .await?;
    let total_pages = ((total_count as f64) / (page_size as f64)).ceil() as i32;
    let ids: Vec<Uuid> = sqlx::query_scalar(
        "SELECT DISTINCT work_id FROM public.chapters ORDER BY work_id LIMIT $1 OFFSET $2",
    )
    .bind(page_size as i64)
    .bind(offset)
    .fetch_all(&db)
    .await?;
    Ok(Json(SuccessResponse {
        result: "Success".to_string(),
        status: 200,
        data: MangaIdsResponse {
            items: ids,
            total_items: total_count,
            current_page: page,
            page_size,
            total_pages,
        },
    }))
}

/// GET /v2/manga/{id}/chapter-ids
#[utoipa::path(get, path = "/v2/manga/{id}/chapter-ids", tag = "manga", responses(
    (status = 200, description = "Success", body = SuccessResponse<ItemsResponse<ChapterIdsResponse>>),
    (status = 500, description = "Internal error", body = ErrorResponseTemplate),
))]
pub async fn chapter_ids(
    Path(id): Path<Uuid>,
    State(db): State<DbPool>,
) -> Result<Json<SuccessResponse<ItemsResponse<ChapterIdsResponse>>>, ApiError> {
    #[derive(Debug, sqlx::FromRow)]
    #[allow(dead_code)]
    struct IdRow {
        work_id: Uuid,
        number: f64,
    }

    let rows: Vec<IdRow> = sqlx::query_as::<_, IdRow>(
        "SELECT work_id, number::double precision AS number FROM public.chapters WHERE work_id = $1 ORDER BY number DESC",
    )
    .bind(id)
    .fetch_all(&db)
    .await?;

    let chapter_ids: Vec<f64> = rows.iter().map(|r| r.number).collect();
    let items = vec![ChapterIdsResponse {
        work_id: id,
        chapter_ids,
    }];

    Ok(Json(SuccessResponse {
        result: "Success".to_string(),
        status: 200,
        data: ItemsResponse { items },
    }))
}

/// GET /v2/manga/mal/{malId}
#[utoipa::path(get, path = "/v2/manga/mal/{id}", tag = "manga", responses(
    (status = 200, description = "Success", body = SuccessResponse<MangaDetailResponse>),
    (status = 404, description = "Not found", body = ErrorResponseTemplate),
    (status = 500, description = "Internal error", body = ErrorResponseTemplate),
))]
pub async fn by_mal_id(
    Path(mal_id): Path<i32>,
    State(db): State<DbPool>,
) -> Result<Json<SuccessResponse<MangaDetailResponse>>, ApiError> {
    let work_id: Uuid = sqlx::query_scalar(
        "SELECT w.id FROM public.works w \
         JOIN public.work_trackers wt ON wt.work_id = w.id \
         JOIN public.trackers t ON t.id = wt.tracker_id \
         WHERE t.code = 'myanimelist' AND wt.tracker_work_id = $1",
    )
    .bind(mal_id.to_string())
    .fetch_optional(&db)
    .await?
    .ok_or(ApiError::not_found("Manga not found"))?;

    // Tracker lookup resolved the work ID; projection and chapters are then
    // independent and run concurrently.
    let manga_fut = async {
        sqlx::query_as::<_, MangaListRow>(MANGA_BY_ID_SQL)
            .bind(work_id)
            .fetch_optional(&db)
            .await
            .map_err(ApiError::from)
    };
    let chapters_fut = fetch_chapters_for_work(&db, work_id);
    let (manga_row, chapters) = tokio::try_join!(manga_fut, chapters_fut)?;
    let manga_row: MangaListRow = manga_row.ok_or(ApiError::not_found("Manga not found"))?;

    Ok(Json(SuccessResponse {
        result: "Success".to_string(),
        status: 200,
        data: flatten_manga_detail(manga_row, chapters),
    }))
}

/// GET /v2/manga/ani/{aniId}
#[utoipa::path(get, path = "/v2/manga/ani/{id}", tag = "manga", responses(
    (status = 200, description = "Success", body = SuccessResponse<MangaDetailResponse>),
    (status = 404, description = "Not found", body = ErrorResponseTemplate),
    (status = 500, description = "Internal error", body = ErrorResponseTemplate),
))]
pub async fn by_ani_id(
    Path(ani_id): Path<i32>,
    State(db): State<DbPool>,
) -> Result<Json<SuccessResponse<MangaDetailResponse>>, ApiError> {
    let work_id: Uuid = sqlx::query_scalar(
        "SELECT w.id FROM public.works w \
         JOIN public.work_trackers wt ON wt.work_id = w.id \
         JOIN public.trackers t ON t.id = wt.tracker_id \
         WHERE t.code = 'anilist' AND wt.tracker_work_id = $1",
    )
    .bind(ani_id.to_string())
    .fetch_optional(&db)
    .await?
    .ok_or(ApiError::not_found("Manga not found"))?;

    // Tracker lookup resolved the work ID; projection and chapters are then
    // independent and run concurrently.
    let manga_fut = async {
        sqlx::query_as::<_, MangaListRow>(MANGA_BY_ID_SQL)
            .bind(work_id)
            .fetch_optional(&db)
            .await
            .map_err(ApiError::from)
    };
    let chapters_fut = fetch_chapters_for_work(&db, work_id);
    let (manga_row, chapters) = tokio::try_join!(manga_fut, chapters_fut)?;
    let manga_row = manga_row.ok_or(ApiError::not_found("Manga not found"))?;

    Ok(Json(SuccessResponse {
        result: "Success".to_string(),
        status: 200,
        data: flatten_manga_detail(manga_row, chapters),
    }))
}

fn flatten_manga_detail(row: MangaListRow, chapters: Vec<MangaChapter>) -> MangaDetailResponse {
    let m: MangaResponse = row.into();
    MangaDetailResponse {
        id: m.id,
        title: m.title,
        cover: m.cover,
        description: m.description,
        status: m.status,
        manga_type: m.manga_type,
        authors: m.authors,
        genres: m.genres,
        views: m.views,
        rating: m.rating,
        alternative_titles: m.alternative_titles,
        trackers: m.trackers.clone(),
        preferred_scanlator_id: m.preferred_scanlator_id,
        created_at: m.created_at,
        updated_at: m.updated_at,
        chapters,
    }
}

/// GET /v2/manga/batch
#[utoipa::path(get, path = "/v2/manga/batch", tag = "manga", params(BatchParams), responses(
    (status = 200, description = "Success", body = SuccessResponse<ItemsResponse<MangaResponse>>),
    (status = 500, description = "Internal error", body = ErrorResponseTemplate),
))]
pub async fn batch_manga(
    Query(params): Query<BatchParams>,
    State(db): State<DbPool>,
) -> Result<Json<SuccessResponse<ItemsResponse<MangaResponse>>>, ApiError> {
    let ids: Vec<Uuid> = params
        .ids
        .as_deref()
        .map(|s| {
            s.split(',')
                .filter_map(|s| s.trim().parse::<Uuid>().ok())
                .collect()
        })
        .unwrap_or_default();

    if ids.is_empty() {
        return Ok(Json(SuccessResponse {
            result: "Success".to_string(),
            status: 200,
            data: ItemsResponse { items: vec![] },
        }));
    }

    let rows: Vec<MangaListRow> = sqlx::query_as::<_, MangaListRow>(MANGA_BATCH_SQL)
        .bind(&ids)
        .fetch_all(&db)
        .await?;

    let items: Vec<MangaResponse> = rows.into_iter().map(Into::into).collect();

    Ok(Json(SuccessResponse {
        result: "Success".to_string(),
        status: 200,
        data: ItemsResponse { items },
    }))
}

async fn fetch_chapters_for_work(
    db: &DbPool,
    work_id: Uuid,
) -> Result<Vec<MangaChapter>, ApiError> {
    const SQL: &str = "SELECT c.id, c.work_id, c.number::double precision AS number, c.title, c.language_code, c.pages, c.images, \
         c.scanlation_group_id, c.released_at, c.created_at, c.updated_at, sg.name AS scanlation_group_name \
         FROM public.chapters c \
         LEFT JOIN public.scanlation_groups sg ON sg.id = c.scanlation_group_id \
         WHERE c.work_id = $1 \
         ORDER BY c.number DESC";

    let rows: Vec<ChapterRow> = sqlx::query_as::<_, ChapterRow>(SQL)
        .bind(work_id)
        .fetch_all(db)
        .await?;

    Ok(rows.into_iter().map(Into::into).collect())
}

/// GET /v2/manga/{id}/{sub_id}  (sub_id = chapter number)
#[utoipa::path(get, path = "/v2/manga/{id}/{subId}", tag = "manga", params(ChapterDetailParams), responses(
    (status = 200, description = "Success", body = SuccessResponse<ChapterResponse>),
    (status = 404, description = "Not found", body = ErrorResponseTemplate),
    (status = 500, description = "Internal error", body = ErrorResponseTemplate),
))]
pub async fn chapter_detail(
    Path((id, sub_number)): Path<(Uuid, f64)>,
    Query(params): Query<ChapterDetailParams>,
    State(db): State<DbPool>,
) -> Result<Json<SuccessResponse<ChapterResponse>>, ApiError> {
    let mut builder = QueryBuilder::new(
        "SELECT c.id, c.work_id, c.number::double precision AS number, c.title, c.language_code, c.pages, c.images, \
         c.scanlation_group_id, c.released_at, c.created_at, c.updated_at, sg.name AS scanlation_group_name \
         FROM public.chapters c \
         LEFT JOIN public.scanlation_groups sg ON sg.id = c.scanlation_group_id \
         WHERE c.work_id = ",
    );
    builder.push_bind(id);
    builder.push(" AND c.number = ");
    builder.push_bind(sub_number);
    if let Some(sid) = params.scanlation_group_id {
        builder.push(" AND c.scanlation_group_id = ");
        builder.push_bind(sid);
    }

    let chapter_row: ChapterRow = builder
        .build_query_as()
        .fetch_optional(&db)
        .await?
        .ok_or(ApiError::not_found("Chapter not found"))?;

    let chapter_number = chapter_row.number;
    let sg_id = chapter_row.scanlation_group_id;

    // Prev/next navigation: both bounds in one query (same scanlation-group
    // predicate and < / > bounds as before). Runs concurrently with the
    // chapter list, work info, and tracker aggregate.
    let prev_next_fut = async {
        sqlx::query_as::<_, (Option<f64>, Option<f64>)>(
            "SELECT \
               (SELECT number::double precision FROM public.chapters \
                  WHERE work_id = $1 AND scanlation_group_id IS NOT DISTINCT FROM $2 AND number < $3 \
                  ORDER BY number DESC LIMIT 1), \
               (SELECT number::double precision FROM public.chapters \
                  WHERE work_id = $1 AND scanlation_group_id IS NOT DISTINCT FROM $2 AND number > $3 \
                  ORDER BY number ASC LIMIT 1)",
        )
        .bind(id)
        .bind(sg_id)
        .bind(chapter_number)
        .fetch_one(&db)
        .await
        .map_err(ApiError::from)
    };
    let list_fut = async {
        sqlx::query_as::<_, ChapterListRow>(
            "SELECT id, number::double precision AS number, title, scanlation_group_id, pages, created_at, updated_at \
             FROM public.chapters WHERE work_id = $1 ORDER BY number DESC",
        )
        .bind(id)
        .fetch_all(&db)
        .await
        .map_err(ApiError::from)
    };
    let work_fut = async {
        sqlx::query_as::<_, (String, String)>(
            "SELECT title, format FROM public.works WHERE id = $1",
        )
        .bind(id)
        .fetch_optional(&db)
        .await
        .map_err(ApiError::from)
    };
    let trackers_fut = async {
        sqlx::query_scalar::<_, String>(
            "SELECT COALESCE(json_agg(json_build_object('code', t.code, 'id', wt.tracker_work_id) ORDER BY t.code), '[]'::json)::text \
             FROM public.work_trackers wt JOIN public.trackers t ON t.id = wt.tracker_id WHERE wt.work_id = $1",
        )
        .bind(id)
        .fetch_one(&db)
        .await
        .map_err(ApiError::from)
    };

    let (prev_next, list_rows, work_info, trackers) =
        tokio::try_join!(prev_next_fut, list_fut, work_fut, trackers_fut)?;
    let (prev_num, next_num) = prev_next;
    let (work_title, work_format) = work_info.unwrap_or_default();

    // Chapter options list
    // Chapter list for all chapters
    let chapters: Vec<MangaChapter> = list_rows
        .into_iter()
        .map(|r| {
            let sg_id = r.scanlation_group_id;
            MangaChapter {
                id: r.id,
                title: r.title.clone().unwrap_or_else(|| format!("Chapter {}", r.number)),
                number: r.number,
                scanlator_id: sg_id.unwrap_or(0),
                pages: r.pages.map(|p| p as i32),
                created_at: r.created_at,
                updated_at: r.updated_at,
            }
        })
        .collect();

    Ok(Json(SuccessResponse {
        result: "Success".to_string(),
        status: 200,
        data: ChapterResponse {
            id: chapter_row.id,
            manga_type: WorkFormat::from(&work_format as &str),
            pages: chapter_row.pages.unwrap_or(0) as i32,
            title: chapter_row
                .title
                .unwrap_or_else(|| format!("Chapter {}", chapter_row.number)),
            images: chapter_row.images,
            number: chapter_row.number,
            chapters,
            scanlator: sg_id.map(|sid| Scanlator {
                id: sid,
                name: chapter_row.scanlation_group_name.clone().unwrap_or_default(),
            }),
            work_id: id,
            work_title,
            last_chapter: prev_num.map(|n| ChapterNavigation {
                number: n,
                scanlator_id: sg_id.unwrap_or(0),
            }),
            next_chapter: next_num.map(|n| ChapterNavigation {
                number: n,
                scanlator_id: sg_id.unwrap_or(0),
            }),
            trackers: serde_json::from_str(&trackers).unwrap_or_default(),
        },
    }))
}

#[derive(Debug, Deserialize, utoipa::ToSchema, utoipa::IntoParams)]
pub struct RateBody {
    pub rating: i16,
}

#[derive(Debug, Deserialize, utoipa::ToSchema)]
pub struct BatchRateItem {
    #[serde(rename = "mangaId")]
    pub work_id: Uuid,
    pub rating: i16,
}

#[derive(Debug, Deserialize, utoipa::ToSchema, utoipa::IntoParams)]
pub struct BatchRateBody {
    pub ratings: Vec<BatchRateItem>,
}

#[derive(Debug, Deserialize, utoipa::ToSchema, utoipa::IntoParams)]
pub struct ViewBody {
    #[serde(rename = "saveUserId")]
    pub save_user_id: Option<bool>,
}

/// POST /v2/manga/{id}/view
#[utoipa::path(post, path = "/v2/manga/{id}/view", tag = "manga", request_body = ViewBody, responses(
    (status = 200, description = "Success"),
    (status = 500, description = "Internal error", body = ErrorResponseTemplate),
))]
pub async fn record_view(
    OptionalAuthUser(user): OptionalAuthUser,
    Path(id): Path<Uuid>,
    State(db): State<DbPool>,
    axum::extract::ConnectInfo(addr): axum::extract::ConnectInfo<std::net::SocketAddr>,
) -> Result<Json<SuccessResponse<serde_json::Value>>, ApiError> {
    let ip = addr.ip().to_string();

    // Dedup: check if same IP viewed in last 24h
    let recent: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM public.work_views WHERE work_id = $1 AND ip = $2::inet AND viewed_at > now() - interval '24 hours')",
    )
    .bind(id)
    .bind(&ip)
    .fetch_one(&db)
    .await?;

    if !recent {
        sqlx::query(
            "INSERT INTO public.work_views (work_id, ip, user_id) VALUES ($1, $2::inet, $3)",
        )
        .bind(id)
        .bind(&ip)
        .bind(user.map(|u| u.id))
        .execute(&db)
        .await?;
    }

    Ok(Json(SuccessResponse {
        result: "Success".to_string(),
        status: 200,
        data: serde_json::json!({}),
    }))
}

/// GET /v2/manga/viewed
#[utoipa::path(get, path = "/v2/manga/viewed", tag = "manga", params(PopularParams), responses(
    (status = 200, description = "Success", body = SuccessResponse<Vec<MangaResponse>>),
    (status = 401, description = "Unauthorized", body = ErrorResponseTemplate),
    (status = 500, description = "Internal error", body = ErrorResponseTemplate),
))]
pub async fn recently_viewed(
    user: AuthUser,
    Query(params): Query<PopularParams>,
    State(db): State<DbPool>,
) -> Result<Json<SuccessResponse<Vec<MangaResponse>>>, ApiError> {
    let limit = params.limit.unwrap_or(20).clamp(1, 100);

    let work_ids: Vec<Uuid> = sqlx::query_scalar(
        "SELECT DISTINCT ON (work_id) work_id \
         FROM public.work_views \
         WHERE user_id = $1 \
         ORDER BY work_id, viewed_at DESC \
         LIMIT $2",
    )
    .bind(&user.id)
    .bind(limit as i64)
    .fetch_all(&db)
    .await?;

    if work_ids.is_empty() {
        return Ok(Json(SuccessResponse {
            result: "Success".to_string(),
            status: 200,
            data: vec![],
        }));
    }

    let rows: Vec<MangaListRow> = sqlx::query_as::<_, MangaListRow>(
        "SELECT w.id, w.title, w.description, w.status, w.format, w.genres, \
         w.view_count, w.score::double precision AS score, w.preferred_scanlation_group_id, \
         w.created_at, w.updated_at, \
         cov.url AS cover_url, cov.thumbhash AS cover_thumbhash, auth.authors, alt.alternative_titles, \
         tr.trackers, r.rating_count, r.average_rating \
         FROM public.works w \
         LEFT JOIN LATERAL (SELECT url, thumbhash FROM public.covers WHERE work_id = w.id AND is_preferred = TRUE LIMIT 1) cov ON TRUE \
         LEFT JOIN LATERAL (SELECT COALESCE(ARRAY_AGG(a.name ORDER BY wa.position, a.name), '{}'::text[]) AS authors \
           FROM public.work_authors wa JOIN public.authors a ON a.id = wa.author_id WHERE wa.work_id = w.id) auth ON TRUE \
          LEFT JOIN LATERAL (SELECT COALESCE(json_agg(json_build_object('title', wt.title, 'languageCode', wt.language_code, 'titleType', wt.title_type) ORDER BY wt.language_code, wt.title_type), '[]'::json)::text AS alternative_titles \
            FROM public.work_titles wt WHERE wt.work_id = w.id) alt ON TRUE \
          LEFT JOIN LATERAL (SELECT COALESCE(json_agg(json_build_object('code', t.code, 'id', wt.tracker_work_id) ORDER BY t.code), '[]'::json)::text AS trackers \
            FROM public.work_trackers wt JOIN public.trackers t ON t.id = wt.tracker_id WHERE wt.work_id = w.id) tr ON TRUE \
          LEFT JOIN LATERAL (SELECT COUNT(*)::bigint AS rating_count, COALESCE(AVG(rating), 0)::double precision AS average_rating \
           FROM public.work_ratings WHERE work_id = w.id) r ON TRUE \
         WHERE w.id = ANY($1) ORDER BY w.title",
    )
    .bind(&work_ids)
    .fetch_all(&db)
    .await?;

    Ok(Json(SuccessResponse {
        result: "Success".to_string(),
        status: 200,
        data: rows.into_iter().map(Into::into).collect(),
    }))
}

/// POST /v2/manga/{id}/rate
#[utoipa::path(post, path = "/v2/manga/{id}/rate", tag = "manga", responses(
    (status = 200, description = "Success"),
    (status = 400, description = "Bad request", body = ErrorResponseTemplate),
    (status = 401, description = "Unauthorized", body = ErrorResponseTemplate),
    (status = 500, description = "Internal error", body = ErrorResponseTemplate),
))]
pub async fn rate_manga(
    user: AuthUser,
    Path(id): Path<Uuid>,
    State(db): State<DbPool>,
    Json(body): Json<RateBody>,
) -> Result<Json<SuccessResponse<serde_json::Value>>, ApiError> {
    if body.rating < 1 || body.rating > 10 {
        return Err(ApiError::bad_request("Rating must be between 1 and 10"));
    }

    sqlx::query(
        "INSERT INTO public.work_ratings (user_id, work_id, rating, updated_at) \
         VALUES ($1, $2, $3, now()) \
         ON CONFLICT (user_id, work_id) \
         DO UPDATE SET rating = $3, updated_at = now()",
    )
    .bind(&user.id)
    .bind(id)
    .bind(body.rating)
    .execute(&db)
    .await?;

    Ok(Json(SuccessResponse {
        result: "Success".to_string(),
        status: 200,
        data: serde_json::json!({}),
    }))
}

/// GET /v2/manga/{id}/rating
#[utoipa::path(get, path = "/v2/manga/{id}/rating", tag = "manga", responses(
    (status = 200, description = "Success", body = SuccessResponse<RatingResponse>),
    (status = 401, description = "Unauthorized", body = ErrorResponseTemplate),
    (status = 500, description = "Internal error", body = ErrorResponseTemplate),
))]
pub async fn get_rating(
    user: AuthUser,
    Path(id): Path<Uuid>,
    State(db): State<DbPool>,
) -> Result<Json<SuccessResponse<RatingResponse>>, ApiError> {
    let rating: Option<i16> = sqlx::query_scalar(
        "SELECT rating FROM public.work_ratings WHERE user_id = $1 AND work_id = $2",
    )
    .bind(&user.id)
    .bind(id)
    .fetch_optional(&db)
    .await?
    .flatten();

    Ok(Json(SuccessResponse {
        result: "Success".to_string(),
        status: 200,
        data: RatingResponse {
            rating: rating.map(|r| r as i32),
        },
    }))
}

/// DELETE /v2/manga/{id}/rate
#[utoipa::path(delete, path = "/v2/manga/{id}/rate", tag = "manga", responses(
    (status = 200, description = "Success"),
    (status = 401, description = "Unauthorized", body = ErrorResponseTemplate),
    (status = 500, description = "Internal error", body = ErrorResponseTemplate),
))]
pub async fn delete_rating(
    user: AuthUser,
    Path(id): Path<Uuid>,
    State(db): State<DbPool>,
) -> Result<Json<SuccessResponse<serde_json::Value>>, ApiError> {
    sqlx::query("DELETE FROM public.work_ratings WHERE user_id = $1 AND work_id = $2")
        .bind(&user.id)
        .bind(id)
        .execute(&db)
        .await?;

    Ok(Json(SuccessResponse {
        result: "Success".to_string(),
        status: 200,
        data: serde_json::json!({}),
    }))
}

/// POST /v2/manga/rate/batch
#[utoipa::path(post, path = "/v2/manga/rate/batch", tag = "manga", responses(
    (status = 200, description = "Success"),
    (status = 400, description = "Bad request", body = ErrorResponseTemplate),
    (status = 401, description = "Unauthorized", body = ErrorResponseTemplate),
    (status = 500, description = "Internal error", body = ErrorResponseTemplate),
))]
pub async fn batch_rate(
    user: AuthUser,
    State(db): State<DbPool>,
    Json(body): Json<BatchRateBody>,
) -> Result<Json<SuccessResponse<serde_json::Value>>, ApiError> {
    if body.ratings.len() > 50 {
        return Err(ApiError::bad_request("Max 50 ratings per batch"));
    }

    for item in &body.ratings {
        if item.rating < 1 || item.rating > 10 {
            return Err(ApiError::bad_request("Rating must be between 1 and 10"));
        }

        sqlx::query(
            "INSERT INTO public.work_ratings (user_id, work_id, rating, updated_at) \
             VALUES ($1, $2, $3, now()) \
             ON CONFLICT (user_id, work_id) \
             DO UPDATE SET rating = $3, updated_at = now()",
        )
        .bind(&user.id)
        .bind(item.work_id)
        .bind(item.rating)
        .execute(&db)
        .await?;
    }

    Ok(Json(SuccessResponse {
        result: "Success".to_string(),
        status: 200,
        data: serde_json::json!({}),
    }))
}
