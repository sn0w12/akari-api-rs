use axum::Json;
use axum::extract::{Path, Query, State};
use chrono::{DateTime, Utc};
use serde::Deserialize;
use sqlx::QueryBuilder;
use uuid::Uuid;

use crate::auth::AuthUser;
use crate::db::DbPool;
use crate::error::{ApiError, ErrorResponseTemplate};
use crate::models::bookmark::{
    BookmarkBatchBody, BookmarkDetailResponse, BookmarkResponse, DayOfWeekReadCount, GenreCount,
    HistoryBucket, HourReadCount, PaginatedBookmarkResponse, ReadingHistoryResponse,
    ReadingStatsResponse,
};
use crate::response::SuccessResponse;

#[derive(Debug, Deserialize, utoipa::IntoParams)]
pub struct BookmarkListParams {
    pub page: Option<i32>,
    pub page_size: Option<i32>,
}

#[derive(Debug, Deserialize, utoipa::IntoParams)]
pub struct BookmarkSearchParams {
    pub query: String,
    pub page: Option<i32>,
    pub page_size: Option<i32>,
}

#[derive(Debug, Deserialize, utoipa::ToSchema, utoipa::IntoParams)]
pub struct BookmarkUpsertBody {
    #[serde(rename = "chapterNumber")]
    pub chapter_number: Option<f64>,
}

#[derive(Debug, Deserialize, utoipa::IntoParams)]
pub struct HistoryParams {
    pub bucket: Option<HistoryBucket>,
    pub range: Option<i32>,
}

fn pagination(page: Option<i32>, page_size: Option<i32>) -> (i32, i32, i64) {
    let p = page.unwrap_or(1).max(1);
    let ps = page_size.unwrap_or(20).clamp(1, 100);
    (p, ps, ((p - 1) as i64) * (ps as i64))
}

use crate::models::chapter::MangaChapter;
use crate::models::manga_type::WorkFormat;



#[derive(Debug, sqlx::FromRow)]
struct BookmarkRow {
    work_id: Uuid,
    title: String,
    description: String,
    status: String,
    #[sqlx(rename = "format")]
    manga_type: String,
    genres: Vec<String>,
    authors: Vec<String>,
    alternative_titles: Vec<String>,
    view_count: i64,
    score: Option<f64>,
    mal_id: Option<String>,
    ani_id: Option<String>,
    cover: Option<String>,
    work_created_at: DateTime<Utc>,
    work_updated_at: DateTime<Utc>,
    // Last read chapter
    lr_id: Option<Uuid>,
    lr_title: Option<String>,
    lr_number: Option<f64>,
    lr_pages: Option<i16>,
    lr_scanlation_group_id: Option<i32>,
    lr_created_at: Option<DateTime<Utc>>,
    lr_updated_at: Option<DateTime<Utc>>,
    // Latest chapter
    lt_id: Option<Uuid>,
    lt_title: Option<String>,
    lt_number: Option<f64>,
    lt_pages: Option<i16>,
    lt_scanlation_group_id: Option<i32>,
    lt_created_at: Option<DateTime<Utc>>,
    lt_updated_at: Option<DateTime<Utc>>,
    // Next chapter
    nx_id: Option<Uuid>,
    nx_title: Option<String>,
    nx_number: Option<f64>,
    nx_pages: Option<i16>,
    nx_scanlation_group_id: Option<i32>,
    nx_created_at: Option<DateTime<Utc>>,
    nx_updated_at: Option<DateTime<Utc>>,
    chapters_behind: i32,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

impl BookmarkRow {
    fn last_read(&self) -> MangaChapter {
        MangaChapter {
            id: self.lr_id.unwrap_or_default(),
            title: self.lr_title.clone().unwrap_or_else(|| format!("Chapter {}", self.lr_number.unwrap_or(0.0))),
            number: self.lr_number.unwrap_or(0.0),
            scanlator_id: self.lr_scanlation_group_id.unwrap_or(0), pages: self.lr_pages.map(|p| p as i32),
            created_at: self.lr_created_at.unwrap_or(self.updated_at),
            updated_at: self.lr_updated_at.unwrap_or(self.updated_at),
        }
    }
    fn latest_ch(&self) -> MangaChapter {
        MangaChapter {
            id: self.lt_id.unwrap_or_default(),
            title: self.lt_title.clone().unwrap_or_else(|| format!("Chapter {}", self.lt_number.unwrap_or(0.0))),
            number: self.lt_number.unwrap_or(0.0),
            scanlator_id: self.lt_scanlation_group_id.unwrap_or(0), pages: self.lt_pages.map(|p| p as i32),
            created_at: self.lt_created_at.unwrap_or(self.updated_at),
            updated_at: self.lt_updated_at.unwrap_or(self.updated_at),
        }
    }
    fn next_ch(&self) -> MangaChapter {
        MangaChapter {
            id: self.nx_id.unwrap_or_default(),
            title: self.nx_title.clone().unwrap_or_else(|| format!("Chapter {}", self.nx_number.unwrap_or(0.0))),
            number: self.nx_number.unwrap_or(0.0),
            scanlator_id: self.nx_scanlation_group_id.unwrap_or(0), pages: self.nx_pages.map(|p| p as i32),
            created_at: self.nx_created_at.unwrap_or(self.updated_at),
            updated_at: self.nx_updated_at.unwrap_or(self.updated_at),
        }
    }
}

#[derive(Debug, sqlx::FromRow)]
struct BookmarkDetailRow {
    work_id: Uuid,
    number: Option<f64>,
    title: Option<String>,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

/// GET /v2/bookmarks
#[utoipa::path(get, path = "/v2/bookmarks", tag = "bookmarks", params(BookmarkListParams), responses(
    (status = 200, description = "Success", body = SuccessResponse<PaginatedBookmarkResponse>),
    (status = 401, description = "Unauthorized", body = ErrorResponseTemplate),
    (status = 500, description = "Internal error", body = ErrorResponseTemplate),
))]
pub async fn list_bookmarks(
    user: AuthUser,
    Query(params): Query<BookmarkListParams>,
    State(db): State<DbPool>,
) -> Result<Json<SuccessResponse<PaginatedBookmarkResponse>>, ApiError> {
    let (page, page_size, offset) = pagination(params.page, params.page_size);

    let total_count: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM public.user_library_entries WHERE user_id = $1")
            .bind(&user.id)
            .fetch_one(&db)
            .await?;

    let total_pages = ((total_count as f64) / (page_size as f64)).ceil() as i32;

    let sql = "SELECT ule.work_id, w.title, w.description, w.status, w.format, w.genres, \
         w.view_count, w.score::double precision AS score, \
         w.created_at AS work_created_at, w.updated_at AS work_updated_at, \
         cov.url AS cover, \
         auth.authors, alt.alternative_titles, \
         mal.mal_id, ani.ani_id, \
         c.id AS lr_id, c.title AS lr_title, c.number::double precision AS lr_number, \
         c.pages AS lr_pages, c.scanlation_group_id AS lr_scanlation_group_id, \
         c.created_at AS lr_created_at, c.updated_at AS lr_updated_at, \
         latest.id AS lt_id, latest.title AS lt_title, \
         latest.number::double precision AS lt_number, \
         latest.pages AS lt_pages, latest.scanlation_group_id AS lt_scanlation_group_id, \
         latest.created_at AS lt_created_at, latest.updated_at AS lt_updated_at, \
         next_ch.id AS nx_id, next_ch.title AS nx_title, \
         next_ch.number::double precision AS nx_number, \
         next_ch.pages AS nx_pages, next_ch.scanlation_group_id AS nx_scanlation_group_id, \
         next_ch.created_at AS nx_created_at, next_ch.updated_at AS nx_updated_at, \
         (latest.number - COALESCE(c.number, 0))::integer AS chapters_behind, \
         ule.created_at, ule.updated_at \
         FROM public.user_library_entries ule \
         JOIN public.works w ON w.id = ule.work_id \
         LEFT JOIN LATERAL (SELECT url FROM public.covers WHERE work_id = w.id AND is_preferred = TRUE LIMIT 1) cov ON TRUE \
         LEFT JOIN LATERAL (SELECT COALESCE(ARRAY_AGG(a.name ORDER BY wa.position, a.name), '{}'::text[]) AS authors \
           FROM public.work_authors wa JOIN public.authors a ON a.id = wa.author_id WHERE wa.work_id = w.id) auth ON TRUE \
         LEFT JOIN LATERAL (SELECT COALESCE(ARRAY_AGG(wt.title ORDER BY wt.language_code, wt.title_type), '{}'::text[]) AS alternative_titles \
           FROM public.work_titles wt WHERE wt.work_id = w.id) alt ON TRUE \
         LEFT JOIN LATERAL (SELECT wt.tracker_work_id AS mal_id \
           FROM public.work_trackers wt JOIN public.trackers t ON t.id = wt.tracker_id WHERE t.code = 'myanimelist' AND wt.work_id = w.id) mal ON TRUE \
         LEFT JOIN LATERAL (SELECT wt.tracker_work_id AS ani_id \
           FROM public.work_trackers wt JOIN public.trackers t ON t.id = wt.tracker_id WHERE t.code = 'anilist' AND wt.work_id = w.id) ani ON TRUE \
         LEFT JOIN public.chapters c ON c.id = ule.last_read_chapter_id \
         LEFT JOIN LATERAL (SELECT id, title, number, pages, scanlation_group_id, created_at, updated_at \
           FROM public.chapters WHERE work_id = ule.work_id ORDER BY number DESC LIMIT 1) latest ON TRUE \
         LEFT JOIN LATERAL (SELECT id, title, number, pages, scanlation_group_id, created_at, updated_at \
           FROM public.chapters WHERE work_id = ule.work_id AND number > COALESCE(c.number, 0) ORDER BY number ASC LIMIT 1) next_ch ON TRUE \
         WHERE ule.user_id = $1 \
         ORDER BY ule.updated_at DESC \
         LIMIT $2 OFFSET $3";

    let rows: Vec<BookmarkRow> = sqlx::query_as::<_, BookmarkRow>(sql)
        .bind(&user.id)
        .bind(page_size as i64)
        .bind(offset)
        .fetch_all(&db)
        .await?;

    let items: Vec<BookmarkResponse> = rows
        .into_iter()
        .map(|r| {
            let lr = r.last_read();
            let lt = r.latest_ch();
            let nx = r.next_ch();
            BookmarkResponse {
                work_id: r.work_id,
                title: r.title,
                cover: r.cover.unwrap_or_default(),
                description: r.description,
                status: r.status,
                manga_type: WorkFormat::from(&r.manga_type as &str),
                authors: r.authors,
                genres: r.genres,
                views: r.view_count as i32,
                score: r.score.unwrap_or(0.0),
                mal_id: r.mal_id.and_then(|v| v.parse::<i32>().ok()),
                ani_id: r.ani_id.and_then(|v| v.parse::<i32>().ok()),
                alternative_titles: r.alternative_titles,
                work_created_at: r.work_created_at,
                work_updated_at: r.work_updated_at,
                last_read_chapter: lr,
                latest_chapter: lt,
                next_chapter: nx,
                chapters_behind: r.chapters_behind,
                bookmark_id: r.work_id,
                bookmark_created_at: r.created_at,
                bookmark_updated_at: r.updated_at,
            }
        })
        .collect();

    Ok(Json(SuccessResponse {
        result: "Success".to_string(),
        status: 200,
        data: PaginatedBookmarkResponse {
            items,
            total_items: total_count,
            current_page: page,
            page_size,
            total_pages,
        },
    }))
}

/// GET /v2/bookmarks/search
#[utoipa::path(get, path = "/v2/bookmarks/search", tag = "bookmarks", params(BookmarkSearchParams), responses(
    (status = 200, description = "Success", body = SuccessResponse<PaginatedBookmarkResponse>),
    (status = 401, description = "Unauthorized", body = ErrorResponseTemplate),
    (status = 500, description = "Internal error", body = ErrorResponseTemplate),
))]
pub async fn search_bookmarks(
    user: AuthUser,
    Query(params): Query<BookmarkSearchParams>,
    State(db): State<DbPool>,
) -> Result<Json<SuccessResponse<PaginatedBookmarkResponse>>, ApiError> {
    let (page, page_size, offset) = pagination(params.page, params.page_size);

    let total_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM public.user_library_entries ule \
         JOIN public.works w ON w.id = ule.work_id \
         WHERE ule.user_id = $1 AND w.search_vector @@ plainto_tsquery('english', $2)",
    )
    .bind(&user.id)
    .bind(&params.query)
    .fetch_one(&db)
    .await?;

    let total_pages = ((total_count as f64) / (page_size as f64)).ceil() as i32;

    let sql = "SELECT ule.work_id, w.title, w.description, w.status, w.format, w.genres, \
         w.view_count, w.score::double precision AS score, \
         w.created_at AS work_created_at, w.updated_at AS work_updated_at, \
         cov.url AS cover, \
         auth.authors, alt.alternative_titles, \
         mal.mal_id, ani.ani_id, \
         c.id AS lr_id, c.title AS lr_title, c.number::double precision AS lr_number, \
         c.pages AS lr_pages, c.scanlation_group_id AS lr_scanlation_group_id, \
         c.created_at AS lr_created_at, c.updated_at AS lr_updated_at, \
         latest.id AS lt_id, latest.title AS lt_title, \
         latest.number::double precision AS lt_number, \
         latest.pages AS lt_pages, latest.scanlation_group_id AS lt_scanlation_group_id, \
         latest.created_at AS lt_created_at, latest.updated_at AS lt_updated_at, \
         next_ch.id AS nx_id, next_ch.title AS nx_title, \
         next_ch.number::double precision AS nx_number, \
         next_ch.pages AS nx_pages, next_ch.scanlation_group_id AS nx_scanlation_group_id, \
         next_ch.created_at AS nx_created_at, next_ch.updated_at AS nx_updated_at, \
         (latest.number - COALESCE(c.number, 0))::integer AS chapters_behind, \
         ule.created_at, ule.updated_at \
         FROM public.user_library_entries ule \
         JOIN public.works w ON w.id = ule.work_id \
         LEFT JOIN LATERAL (SELECT url FROM public.covers WHERE work_id = w.id AND is_preferred = TRUE LIMIT 1) cov ON TRUE \
         LEFT JOIN LATERAL (SELECT COALESCE(ARRAY_AGG(a.name ORDER BY wa.position, a.name), '{}'::text[]) AS authors \
           FROM public.work_authors wa JOIN public.authors a ON a.id = wa.author_id WHERE wa.work_id = w.id) auth ON TRUE \
         LEFT JOIN LATERAL (SELECT COALESCE(ARRAY_AGG(wt.title ORDER BY wt.language_code, wt.title_type), '{}'::text[]) AS alternative_titles \
           FROM public.work_titles wt WHERE wt.work_id = w.id) alt ON TRUE \
         LEFT JOIN LATERAL (SELECT wt.tracker_work_id AS mal_id \
           FROM public.work_trackers wt JOIN public.trackers t ON t.id = wt.tracker_id WHERE t.code = 'myanimelist' AND wt.work_id = w.id) mal ON TRUE \
         LEFT JOIN LATERAL (SELECT wt.tracker_work_id AS ani_id \
           FROM public.work_trackers wt JOIN public.trackers t ON t.id = wt.tracker_id WHERE t.code = 'anilist' AND wt.work_id = w.id) ani ON TRUE \
         LEFT JOIN public.chapters c ON c.id = ule.last_read_chapter_id \
         LEFT JOIN LATERAL (SELECT id, title, number, pages, scanlation_group_id, created_at, updated_at \
           FROM public.chapters WHERE work_id = ule.work_id ORDER BY number DESC LIMIT 1) latest ON TRUE \
         LEFT JOIN LATERAL (SELECT id, title, number, pages, scanlation_group_id, created_at, updated_at \
           FROM public.chapters WHERE work_id = ule.work_id AND number > COALESCE(c.number, 0) ORDER BY number ASC LIMIT 1) next_ch ON TRUE \
         WHERE ule.user_id = $1 AND w.search_vector @@ plainto_tsquery('english', $2) \
         ORDER BY ule.updated_at DESC \
         LIMIT $3 OFFSET $4";

    let rows: Vec<BookmarkRow> = sqlx::query_as::<_, BookmarkRow>(sql)
        .bind(&user.id)
        .bind(&params.query)
        .bind(page_size as i64)
        .bind(offset)
        .fetch_all(&db)
        .await?;

    let items: Vec<BookmarkResponse> = rows
        .into_iter()
        .map(|r| {
            let lr = r.last_read();
            let lt = r.latest_ch();
            let nx = r.next_ch();
            BookmarkResponse {
                work_id: r.work_id,
                title: r.title,
                cover: r.cover.unwrap_or_default(),
                description: r.description,
                status: r.status,
                manga_type: WorkFormat::from(&r.manga_type as &str),
                authors: r.authors,
                genres: r.genres,
                views: r.view_count as i32,
                score: r.score.unwrap_or(0.0),
                mal_id: r.mal_id.and_then(|v| v.parse::<i32>().ok()),
                ani_id: r.ani_id.and_then(|v| v.parse::<i32>().ok()),
                alternative_titles: r.alternative_titles,
                work_created_at: r.work_created_at,
                work_updated_at: r.work_updated_at,
                last_read_chapter: lr,
                latest_chapter: lt,
                next_chapter: nx,
                chapters_behind: r.chapters_behind,
                bookmark_id: r.work_id,
                bookmark_created_at: r.created_at,
                bookmark_updated_at: r.updated_at,
            }
        })
        .collect();

    Ok(Json(SuccessResponse {
        result: "Success".to_string(),
        status: 200,
        data: PaginatedBookmarkResponse {
            items,
            total_items: total_count,
            current_page: page,
            page_size,
            total_pages,
        },
    }))
}

/// GET /v2/bookmarks/unread
#[utoipa::path(get, path = "/v2/bookmarks/unread", tag = "bookmarks", responses(
    (status = 200, description = "Success", body = SuccessResponse<i64>),
    (status = 401, description = "Unauthorized", body = ErrorResponseTemplate),
    (status = 500, description = "Internal error", body = ErrorResponseTemplate),
))]
pub async fn unread_count(
    user: AuthUser,
    State(db): State<DbPool>,
) -> Result<Json<SuccessResponse<i64>>, ApiError> {
    let count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM public.user_library_entries ule \
         WHERE ule.user_id = $1 AND EXISTS ( \
           SELECT 1 FROM public.chapters ch \
           WHERE ch.work_id = ule.work_id \
             AND ch.number > (SELECT COALESCE(c.number, 0) FROM public.chapters c WHERE c.id = ule.last_read_chapter_id) \
         )",
    )
    .bind(&user.id)
    .fetch_one(&db)
    .await?;

    Ok(Json(SuccessResponse {
        result: "Success".to_string(),
        status: 200,
        data: count,
    }))
}

/// PUT /v2/bookmarks/{manga_id}
#[utoipa::path(put, path = "/v2/bookmarks/{mangaId}", tag = "bookmarks", responses(
    (status = 200, description = "Success"),
    (status = 401, description = "Unauthorized", body = ErrorResponseTemplate),
    (status = 500, description = "Internal error", body = ErrorResponseTemplate),
))]
pub async fn upsert_bookmark(
    user: AuthUser,
    Path(work_id): Path<Uuid>,
    State(db): State<DbPool>,
    Json(body): Json<BookmarkUpsertBody>,
) -> Result<Json<SuccessResponse<serde_json::Value>>, ApiError> {
    let chapter_id: Option<Uuid> = if let Some(num) = body.chapter_number {
        sqlx::query_scalar(
            "SELECT id FROM public.chapters WHERE work_id = $1 AND number = $2::double precision LIMIT 1",
        )
        .bind(work_id)
        .bind(num)
        .fetch_optional(&db)
        .await?
    } else {
        None
    };

    sqlx::query(
        "INSERT INTO public.user_library_entries (user_id, work_id, last_read_chapter_id, updated_at) \
         VALUES ($1, $2, $3, now()) \
         ON CONFLICT (user_id, work_id) \
         DO UPDATE SET last_read_chapter_id = COALESCE($3, user_library_entries.last_read_chapter_id), updated_at = now()",
    )
    .bind(&user.id)
    .bind(work_id)
    .bind(chapter_id)
    .execute(&db)
    .await?;

    // Also record reading history if chapter_id is set
    if let Some(cid) = chapter_id {
        sqlx::query(
            "INSERT INTO public.reading_history (user_id, work_id, chapter_id, read_at) \
             VALUES ($1, $2, $3, now()) \
             ON CONFLICT DO NOTHING",
        )
        .bind(&user.id)
        .bind(work_id)
        .bind(cid)
        .execute(&db)
        .await
        .ok();
    }

    Ok(Json(SuccessResponse {
        result: "Success".to_string(),
        status: 200,
        data: serde_json::json!({}),
    }))
}

/// DELETE /v2/bookmarks/{manga_id}
#[utoipa::path(delete, path = "/v2/bookmarks/{mangaId}", tag = "bookmarks", responses(
    (status = 200, description = "Success"),
    (status = 401, description = "Unauthorized", body = ErrorResponseTemplate),
    (status = 500, description = "Internal error", body = ErrorResponseTemplate),
))]
pub async fn delete_bookmark(
    user: AuthUser,
    Path(work_id): Path<Uuid>,
    State(db): State<DbPool>,
) -> Result<Json<SuccessResponse<serde_json::Value>>, ApiError> {
    sqlx::query("DELETE FROM public.user_library_entries WHERE user_id = $1 AND work_id = $2")
        .bind(&user.id)
        .bind(work_id)
        .execute(&db)
        .await?;

    Ok(Json(SuccessResponse {
        result: "Success".to_string(),
        status: 200,
        data: serde_json::json!({}),
    }))
}

/// GET /v2/bookmarks/{manga_id}
#[utoipa::path(get, path = "/v2/bookmarks/{mangaId}", tag = "bookmarks", responses(
    (status = 200, description = "Success", body = SuccessResponse<BookmarkDetailResponse>),
    (status = 401, description = "Unauthorized", body = ErrorResponseTemplate),
    (status = 404, description = "Not found", body = ErrorResponseTemplate),
    (status = 500, description = "Internal error", body = ErrorResponseTemplate),
))]
pub async fn get_bookmark(
    user: AuthUser,
    Path(work_id): Path<Uuid>,
    State(db): State<DbPool>,
) -> Result<Json<SuccessResponse<BookmarkDetailResponse>>, ApiError> {
    let sql =         "SELECT ule.work_id, \
         c.number::double precision AS number, \
         c.title, \
         ule.created_at, ule.updated_at \
         FROM public.user_library_entries ule \
         LEFT JOIN public.chapters c ON c.id = ule.last_read_chapter_id \
         WHERE ule.user_id = $1 AND ule.work_id = $2";

    let row: BookmarkDetailRow = sqlx::query_as::<_, BookmarkDetailRow>(sql)
        .bind(&user.id)
        .bind(work_id)
        .fetch_optional(&db)
        .await?
        .ok_or(ApiError::not_found("Bookmark not found"))?;

    Ok(Json(SuccessResponse {
        result: "Success".to_string(),
        status: 200,
        data: BookmarkDetailResponse {
            id: row.work_id,
            title: row.title,
            number: row.number.unwrap_or(0.0),
            pages: None,
            scanlator_id: None,
            created_at: row.created_at,
            updated_at: row.updated_at,
        },
    }))
}

/// POST /v2/bookmarks/batch
#[utoipa::path(post, path = "/v2/bookmarks/batch", tag = "bookmarks", responses(
    (status = 200, description = "Success"),
    (status = 400, description = "Bad request", body = ErrorResponseTemplate),
    (status = 401, description = "Unauthorized", body = ErrorResponseTemplate),
    (status = 500, description = "Internal error", body = ErrorResponseTemplate),
))]
pub async fn batch_upsert(
    user: AuthUser,
    State(db): State<DbPool>,
    Json(body): Json<BookmarkBatchBody>,
) -> Result<Json<SuccessResponse<serde_json::Value>>, ApiError> {
    if body.items.len() > 100 {
        return Err(ApiError::bad_request("Max 100 items per batch"));
    }

    for item in &body.items {
        let chapter_id: Option<Uuid> = if let Some(num) = item.chapter_number {
            sqlx::query_scalar(
                "SELECT id FROM public.chapters WHERE work_id = $1 AND number = $2::double precision LIMIT 1",
            )
            .bind(item.work_id)
            .bind(num)
            .fetch_optional(&db)
            .await?
        } else {
            None
        };

        sqlx::query(
            "INSERT INTO public.user_library_entries (user_id, work_id, last_read_chapter_id, updated_at) \
             VALUES ($1, $2, $3, now()) \
             ON CONFLICT (user_id, work_id) \
             DO UPDATE SET last_read_chapter_id = COALESCE($3, user_library_entries.last_read_chapter_id), updated_at = now()",
        )
        .bind(&user.id)
        .bind(item.work_id)
        .bind(chapter_id)
        .execute(&db)
        .await?;

        if let Some(cid) = chapter_id {
            sqlx::query(
                "INSERT INTO public.reading_history (user_id, work_id, chapter_id, read_at) \
                 VALUES ($1, $2, $3, now()) \
                 ON CONFLICT DO NOTHING",
            )
            .bind(&user.id)
            .bind(item.work_id)
            .bind(cid)
            .execute(&db)
            .await
            .ok();
        }
    }

    Ok(Json(SuccessResponse {
        result: "Success".to_string(),
        status: 200,
        data: serde_json::json!({}),
    }))
}

/// GET /v2/bookmarks/history
#[utoipa::path(get, path = "/v2/bookmarks/history", tag = "bookmarks", params(HistoryParams), responses(
    (status = 200, description = "Success", body = SuccessResponse<Vec<ReadingHistoryResponse>>),
    (status = 401, description = "Unauthorized", body = ErrorResponseTemplate),
    (status = 500, description = "Internal error", body = ErrorResponseTemplate),
))]
pub async fn reading_history(
    user: AuthUser,
    Query(params): Query<HistoryParams>,
    State(db): State<DbPool>,
) -> Result<Json<SuccessResponse<Vec<ReadingHistoryResponse>>>, ApiError> {
    let range = params.range.unwrap_or(30).max(1);

    let bucket_val = match params.bucket.as_ref().unwrap_or(&HistoryBucket::Day) {
        HistoryBucket::Hour => "hour",
        HistoryBucket::Day => "day",
        HistoryBucket::Week => "week",
        HistoryBucket::Month => "month",
        HistoryBucket::Year => "year",
    };

    let mut builder = QueryBuilder::new("SELECT date_trunc('");
    builder.push(bucket_val);
    builder.push(
        "', read_at)::text AS date, COUNT(*)::bigint AS reads \
         FROM public.reading_history \
         WHERE user_id = ",
    );
    builder.push_bind(&user.id);
    builder.push(" AND read_at >= now() - ");
    builder.push_bind(range as i64);
    builder.push(" * interval '1 ");
    builder.push(bucket_val);
    builder.push("' GROUP BY date_trunc('");
    builder.push(bucket_val);
    builder.push("', read_at) ORDER BY date ASC");

    let rows: Vec<(String, i64)> = builder.build_query_as().fetch_all(&db).await?;

    let items: Vec<ReadingHistoryResponse> = rows
        .into_iter()
        .map(|(date, reads)| ReadingHistoryResponse { date, reads })
        .collect();

    Ok(Json(SuccessResponse {
        result: "Success".to_string(),
        status: 200,
        data: items,
    }))
}

/// GET /v2/bookmarks/history/stats
#[utoipa::path(get, path = "/v2/bookmarks/history/stats", tag = "bookmarks", params(HistoryParams), responses(
    (status = 200, description = "Success", body = SuccessResponse<ReadingStatsResponse>),
    (status = 401, description = "Unauthorized", body = ErrorResponseTemplate),
    (status = 500, description = "Internal error", body = ErrorResponseTemplate),
))]
pub async fn reading_stats(
    user: AuthUser,
    Query(params): Query<HistoryParams>,
    State(db): State<DbPool>,
) -> Result<Json<SuccessResponse<ReadingStatsResponse>>, ApiError> {
    let range = params.range.unwrap_or(30).max(1);

    let bucket_val = match params.bucket.as_ref().unwrap_or(&HistoryBucket::Day) {
        HistoryBucket::Hour => "hour",
        HistoryBucket::Day => "day",
        HistoryBucket::Week => "week",
        HistoryBucket::Month => "month",
        HistoryBucket::Year => "year",
    };

    let interval_str = format!("{} {}", range, bucket_val);

    let total_reads: i64 = sqlx::query_scalar(
        "SELECT COUNT(*)::bigint FROM public.reading_history \
         WHERE user_id = $1 AND read_at >= now() - $2::interval",
    )
    .bind(&user.id)
    .bind(&interval_str)
    .fetch_one(&db)
    .await?;

    let unique_manga: i64 = sqlx::query_scalar(
        "SELECT COUNT(DISTINCT work_id) FROM public.reading_history \
         WHERE user_id = $1 AND read_at >= now() - $2::interval",
    )
    .bind(&user.id)
    .bind(&interval_str)
    .fetch_one(&db)
    .await?;

    let avg_per_day = if total_reads > 0 {
        total_reads as f64 / range as f64
    } else {
        0.0
    };

    // Current streak: count consecutive days back from today
    let current_streak: i64 = sqlx::query_scalar(
        "WITH days AS ( \
           SELECT DISTINCT read_at::date AS d FROM public.reading_history \
           WHERE user_id = $1 AND read_at >= now() - $2::interval \
         ) \
         SELECT COALESCE(COUNT(*), 0) FROM ( \
           SELECT d, d - ROW_NUMBER() OVER (ORDER BY d DESC) AS grp \
           FROM days ORDER BY d DESC LIMIT 1000 \
         ) sub \
         WHERE grp = (SELECT d - ROW_NUMBER() OVER (ORDER BY d DESC) FROM days ORDER BY d DESC LIMIT 1)",
    )
    .bind(&user.id)
    .bind(&interval_str)
    .fetch_one(&db)
    .await?;

    // Longest streak
    let longest_streak: i64 = sqlx::query_scalar(
        "WITH days AS ( \
           SELECT DISTINCT read_at::date AS d FROM public.reading_history \
           WHERE user_id = $1 AND read_at >= now() - $2::interval \
         ), groups AS ( \
           SELECT d, d - ROW_NUMBER() OVER (ORDER BY d) AS grp FROM days \
         ) \
         SELECT COALESCE(MAX(COUNT(*)), 0) FROM groups GROUP BY grp",
    )
    .bind(&user.id)
    .bind(&interval_str)
    .fetch_one(&db)
    .await?;

    // Top genres
    let top_genres: Vec<GenreCount> = sqlx::query_as::<_, (String, i64)>(
        "SELECT unnest(w.genres) AS name, COUNT(*)::bigint AS count \
         FROM public.reading_history rh \
         JOIN public.works w ON w.id = rh.work_id \
         WHERE rh.user_id = $1 AND rh.read_at >= now() - $2::interval \
         GROUP BY name ORDER BY count DESC LIMIT 10",
    )
    .bind(&user.id)
    .bind(&interval_str)
    .fetch_all(&db)
    .await?
    .into_iter()
    .map(|(name, count)| GenreCount { name, count })
    .collect();

    // Reads by day of week (0=Sunday, 6=Saturday)
    let reads_by_day_of_week: Vec<DayOfWeekReadCount> = sqlx::query_as::<_, (i32, i64)>(
        "SELECT EXTRACT(DOW FROM read_at)::integer AS dow, COUNT(*)::bigint \
         FROM public.reading_history \
         WHERE user_id = $1 AND read_at >= now() - $2::interval \
         GROUP BY dow ORDER BY dow",
    )
    .bind(&user.id)
    .bind(&interval_str)
    .fetch_all(&db)
    .await?
    .into_iter()
    .map(|(dow, count)| DayOfWeekReadCount { day_of_week: dow, count })
    .collect();

    // Reads by hour (0-23)
    let reads_by_hour: Vec<HourReadCount> = sqlx::query_as::<_, (i32, i64)>(
        "SELECT EXTRACT(HOUR FROM read_at)::integer AS hour, COUNT(*)::bigint \
         FROM public.reading_history \
         WHERE user_id = $1 AND read_at >= now() - $2::interval \
         GROUP BY hour ORDER BY hour",
    )
    .bind(&user.id)
    .bind(&interval_str)
    .fetch_all(&db)
    .await?
    .into_iter()
    .map(|(hour, count)| HourReadCount { hour, count })
    .collect();

    Ok(Json(SuccessResponse {
        result: "Success".to_string(),
        status: 200,
        data: ReadingStatsResponse {
            total_reads,
            unique_manga,
            avg_per_day,
            current_streak,
            longest_streak,
            top_genres,
            reads_by_day_of_week,
            reads_by_hour,
        },
    }))
}
