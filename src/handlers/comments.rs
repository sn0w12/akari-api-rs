use axum::Json;
use axum::extract::{Path, Query, State};
use chrono::{DateTime, Utc};
use serde::Deserialize;
use sqlx::QueryBuilder;
use uuid::Uuid;

use crate::auth::AuthUser;
use crate::banned_words;
use crate::db::DbPool;
use crate::error::{ApiError, ErrorResponseTemplate};
use crate::models::comment::{
    CommentResponse, CommentSortOrder, CommentVoteResponse, PaginatedCommentResponse,
};
use crate::response::ItemsResponse;
use crate::response::SuccessResponse;

#[derive(Debug, Clone, Deserialize, utoipa::IntoParams)]
#[serde(rename_all = "camelCase")]
pub struct CommentListParams {
    pub page: Option<i32>,
    pub page_size: Option<i32>,
    pub sort: Option<CommentSortOrder>,
}

/// GET /v2/comments/{id}
#[utoipa::path(get, path = "/v2/comments/{id}", tag = "comments", params(CommentListParams), responses(
    (status = 200, description = "Success", body = SuccessResponse<PaginatedCommentResponse>),
    (status = 500, description = "Internal error", body = ErrorResponseTemplate),
))]
pub async fn list_comments(
    Path((target_type, target_id)): Path<(String, Uuid)>,
    Query(params): Query<CommentListParams>,
    State(db): State<DbPool>,
) -> Result<Json<SuccessResponse<PaginatedCommentResponse>>, ApiError> {
    let page = params.page.unwrap_or(1).max(1);
    let page_size = params.page_size.unwrap_or(20).clamp(1, 50);
    let offset = ((page - 1) as i64) * (page_size as i64);

    let sort_clause = match params.sort.as_ref() {
        Some(CommentSortOrder::Latest) => "c.created_at DESC",
        Some(CommentSortOrder::Oldest) => "c.created_at ASC",
        _ => "c.upvotes DESC, c.created_at DESC",
    };

    let mut builder = QueryBuilder::new(
        "SELECT c.id, c.target_type, c.target_id, c.user_id, c.content, c.parent_id, \
         c.created_at, c.updated_at, c.edited, c.deleted, c.upvotes, c.downvotes, \
         u.name AS username, u.\"displayUsername\" AS display_username, u.image AS avatar_url, \
         (SELECT COUNT(*) FROM public.comments r WHERE r.parent_id = c.id AND r.deleted = FALSE)::bigint AS reply_count \
         FROM public.comments c \
         JOIN auth.user u ON u.id = c.user_id \
         WHERE c.target_type = ",
    );
    builder.push_bind(&target_type);
    builder.push(" AND c.target_id = ");
    builder.push_bind(target_id);
    builder.push(" AND c.parent_id IS NULL AND c.deleted = FALSE");
    builder.push(" ORDER BY ");
    builder.push(sort_clause);
    builder.push(" LIMIT ");
    builder.push_bind(page_size as i64);
    builder.push(" OFFSET ");
    builder.push_bind(offset);

    let count_fut = sqlx::query_scalar(
        "SELECT COUNT(*) FROM public.comments WHERE target_type = $1 AND target_id = $2 AND parent_id IS NULL AND deleted = FALSE",
    )
    .bind(&target_type)
    .bind(target_id)
    .fetch_one(&db);
    let data_fut = builder.build_query_as().fetch_all(&db);
    let (total_count, rows): (i64, Vec<CommentRow>) = tokio::try_join!(count_fut, data_fut)?;

    let total_pages = ((total_count as f64) / (page_size as f64)).ceil() as i32;

    let items: Vec<CommentResponse> = rows
        .into_iter()
        .map(|r| CommentResponse {
            id: r.id,
            target_type: r.target_type,
            target_id: r.target_id,
            user_profile: crate::models::comment::UserProfile {
                id: r.user_id.clone(),
                username: r.username.clone(),
                display_name: r.display_username.clone().unwrap_or(r.username),
                role: "user".to_string(),
                banned: false,
            },
            content: r.content,
            parent_id: r.parent_id,
            created_at: r.created_at,
            updated_at: r.updated_at,
            edited: r.edited,
            deleted: r.deleted,
            upvotes: r.upvotes,
            downvotes: r.downvotes,
            reply_count: r.reply_count,
        })
        .collect();

    Ok(Json(SuccessResponse {
        result: "Success".to_string(),
        status: 200,
        data: PaginatedCommentResponse {
            items,
            total_items: total_count,
            current_page: page,
            page_size,
            total_pages,
        },
    }))
}

#[derive(Debug, sqlx::FromRow)]
#[allow(dead_code)]
struct CommentRow {
    id: Uuid,
    target_type: String,
    target_id: Uuid,
    user_id: String,
    content: String,
    parent_id: Option<Uuid>,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
    edited: bool,
    deleted: bool,
    upvotes: i32,
    downvotes: i32,
    username: String,
    display_username: Option<String>,
    avatar_url: Option<String>,
    reply_count: Option<i64>,
}

#[derive(Debug, Deserialize, utoipa::ToSchema)]
pub struct CreateCommentBody {
    #[serde(rename = "targetType")]
    pub target_type: String,
    pub content: String,
    #[serde(rename = "parentId")]
    pub parent_id: Option<Uuid>,
}

#[derive(Debug, Deserialize, utoipa::ToSchema)]
pub struct UpdateCommentBody {
    pub content: String,
}

#[derive(Debug, Deserialize, utoipa::ToSchema)]
pub struct VoteBody {
    pub value: i16,
}

#[derive(Debug, Deserialize, utoipa::ToSchema)]
pub struct ReportBody {
    pub reason: String,
    pub description: Option<String>,
}

/// GET /v2/comments/{id} — list comments for a target
#[utoipa::path(get, path = "/v2/comments/{id}", tag = "comments", params(CommentListParams), responses(
    (status = 200, description = "Success", body = SuccessResponse<PaginatedCommentResponse>),
    (status = 500, description = "Internal error", body = ErrorResponseTemplate),
))]
pub async fn list_comments_by_target(
    Path(target_id): Path<Uuid>,
    Query(params): Query<CommentListParams>,
    State(db): State<DbPool>,
) -> Result<Json<SuccessResponse<PaginatedCommentResponse>>, ApiError> {
    list_comments(Path(("work".into(), target_id)), Query(params), State(db)).await
}

/// GET /v2/comments/{commentId}/replies
#[utoipa::path(get, path = "/v2/comments/{commentId}/replies", tag = "comments", responses(
    (status = 200, description = "Success", body = SuccessResponse<Vec<CommentResponse>>),
    (status = 500, description = "Internal error", body = ErrorResponseTemplate),
))]
pub async fn get_comment_replies(
    Path(comment_id): Path<Uuid>,
    State(db): State<DbPool>,
) -> Result<Json<SuccessResponse<Vec<CommentResponse>>>, ApiError> {
    let rows: Vec<CommentRow> = sqlx::query_as::<_, CommentRow>(
        "SELECT c.id, c.target_type, c.target_id, c.user_id, c.content, c.parent_id, \
         c.created_at, c.updated_at, c.edited, c.deleted, c.upvotes, c.downvotes, \
         u.name AS username, u.\"displayUsername\" AS display_username, u.image AS avatar_url, \
         (SELECT COUNT(*) FROM public.comments r WHERE r.parent_id = c.id AND r.deleted = FALSE)::bigint AS reply_count \
         FROM public.comments c \
         JOIN auth.user u ON u.id = c.user_id \
          WHERE c.parent_id = $1 AND c.deleted = FALSE \
         ORDER BY c.created_at ASC",
    )
    .bind(comment_id)
    .fetch_all(&db)
    .await?;

    let items: Vec<CommentResponse> = rows
        .into_iter()
        .map(|r| CommentResponse {
            id: r.id,
            target_type: r.target_type,
            target_id: r.target_id,
            user_profile: crate::models::comment::UserProfile {
                id: r.user_id.clone(),
                username: r.username.clone(),
                display_name: r.display_username.clone().unwrap_or(r.username),
                role: "user".to_string(),
                banned: false,
            },
            content: r.content,
            parent_id: r.parent_id,
            created_at: r.created_at,
            updated_at: r.updated_at,
            edited: r.edited,
            deleted: r.deleted,
            upvotes: r.upvotes,
            downvotes: r.downvotes,
            reply_count: r.reply_count,
        })
        .collect();
    Ok(Json(SuccessResponse {
        result: "Success".to_string(),
        status: 200,
        data: items,
    }))
}

#[derive(Debug, sqlx::FromRow)]
#[allow(dead_code)]
struct InsertCommentRow {
    id: Uuid,
    target_type: String,
    target_id: Uuid,
    user_id: String,
    content: String,
    parent_id: Option<Uuid>,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
    edited: bool,
    deleted: bool,
    upvotes: i32,
    downvotes: i32,
}

/// POST /v2/comments/{id}
#[utoipa::path(post, path = "/v2/comments/{id}", tag = "comments", responses(
    (status = 200, description = "Success", body = SuccessResponse<CommentResponse>),
    (status = 400, description = "Bad request", body = ErrorResponseTemplate),
    (status = 401, description = "Unauthorized", body = ErrorResponseTemplate),
    (status = 500, description = "Internal error", body = ErrorResponseTemplate),
))]
pub async fn create_comment(
    user: AuthUser,
    Path(target_id): Path<Uuid>,
    State(db): State<DbPool>,
    Json(body): Json<CreateCommentBody>,
) -> Result<Json<SuccessResponse<CommentResponse>>, ApiError> {
    if body.content.is_empty() || body.content.len() > 1000 {
        return Err(ApiError::bad_request("Content must be 1-1000 characters"));
    }

    if banned_words::contains_banned_content(&body.content) {
        return Err(ApiError::bad_request("Comment contains prohibited content"));
    }

    if body.target_type != "chapter" && body.target_type != "work" {
        return Err(ApiError::bad_request(
            "targetType must be 'chapter' or 'work'",
        ));
    }

    // Validate parent if provided
    if let Some(parent_id) = body.parent_id {
        let parent_valid: bool = sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM public.comments WHERE id = $1 AND deleted = FALSE AND target_id = $2 AND target_type = $3)",
        )
        .bind(parent_id)
        .bind(target_id)
        .bind(&body.target_type)
        .fetch_one(&db)
        .await?;

        if !parent_valid {
            return Err(ApiError::bad_request("Parent comment not found or invalid"));
        }
    }

    let row: InsertCommentRow = sqlx::query_as::<_, InsertCommentRow>(
        "INSERT INTO public.comments (target_type, target_id, user_id, parent_id, content) \
         VALUES ($1, $2, $3, $4, $5) \
         RETURNING id, target_type, target_id, user_id, content, parent_id, \
                   created_at, updated_at, edited, deleted, upvotes, downvotes",
    )
    .bind(&body.target_type)
    .bind(target_id)
    .bind(&user.id)
    .bind(body.parent_id)
    .bind(&body.content)
    .fetch_one(&db)
    .await?;

    let resp = CommentResponse {
        id: row.id,
        target_type: row.target_type,
        target_id: row.target_id,
        user_profile: crate::models::comment::UserProfile {
            id: user.id.clone(),
            username: user.username.clone(),
            display_name: user
                .display_name
                .clone()
                .unwrap_or_else(|| user.username.clone()),
            role: user.role.clone().unwrap_or_else(|| "user".to_string()),
            banned: user.banned.unwrap_or(false),
        },
        content: row.content,
        parent_id: row.parent_id,
        created_at: row.created_at,
        updated_at: row.updated_at,
        edited: row.edited,
        deleted: row.deleted,
        upvotes: row.upvotes,
        downvotes: row.downvotes,
        reply_count: None,
    };

    Ok(Json(SuccessResponse {
        result: "Success".to_string(),
        status: 201,
        data: resp,
    }))
}

/// PUT /v2/comments/{commentId}
#[utoipa::path(put, path = "/v2/comments/{commentId}", tag = "comments", responses(
    (status = 200, description = "Success"),
    (status = 400, description = "Bad request", body = ErrorResponseTemplate),
    (status = 401, description = "Unauthorized", body = ErrorResponseTemplate),
    (status = 404, description = "Not found", body = ErrorResponseTemplate),
    (status = 500, description = "Internal error", body = ErrorResponseTemplate),
))]
pub async fn update_comment(
    user: AuthUser,
    Path(comment_id): Path<Uuid>,
    State(db): State<DbPool>,
    Json(body): Json<UpdateCommentBody>,
) -> Result<Json<SuccessResponse<serde_json::Value>>, ApiError> {
    if body.content.is_empty() || body.content.len() > 1000 {
        return Err(ApiError::bad_request("Content must be 1-1000 characters"));
    }

    if banned_words::contains_banned_content(&body.content) {
        return Err(ApiError::bad_request("Comment contains prohibited content"));
    }

    let result = sqlx::query(
        "UPDATE public.comments SET content = $1, edited = TRUE, updated_at = now() \
         WHERE id = $2 AND user_id = $3 AND deleted = FALSE",
    )
    .bind(&body.content)
    .bind(comment_id)
    .bind(&user.id)
    .execute(&db)
    .await?;

    if result.rows_affected() == 0 {
        return Err(ApiError::not_found("Comment not found or not yours"));
    }

    Ok(Json(SuccessResponse {
        result: "Success".to_string(),
        status: 200,
        data: serde_json::json!({}),
    }))
}

/// DELETE /v2/comments/{commentId}
#[utoipa::path(delete, path = "/v2/comments/{commentId}", tag = "comments", responses(
    (status = 200, description = "Success"),
    (status = 401, description = "Unauthorized", body = ErrorResponseTemplate),
    (status = 500, description = "Internal error", body = ErrorResponseTemplate),
))]
pub async fn delete_comment(
    user: AuthUser,
    Path(comment_id): Path<Uuid>,
    State(db): State<DbPool>,
) -> Result<Json<SuccessResponse<serde_json::Value>>, ApiError> {
    // Check if comment has replies
    let has_replies: bool =
        sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM public.comments WHERE parent_id = $1)")
            .bind(comment_id)
            .fetch_one(&db)
            .await?;

    if has_replies {
        // Soft delete
        sqlx::query(
            "UPDATE public.comments SET deleted = TRUE, content = '[deleted]', updated_at = now() \
             WHERE id = $1 AND user_id = $2",
        )
        .bind(comment_id)
        .bind(&user.id)
        .execute(&db)
        .await?;
    } else {
        // Hard delete
        sqlx::query("DELETE FROM public.comments WHERE id = $1 AND user_id = $2")
            .bind(comment_id)
            .bind(&user.id)
            .execute(&db)
            .await?;
    }

    Ok(Json(SuccessResponse {
        result: "Success".to_string(),
        status: 201,
        data: serde_json::json!({}),
    }))
}

/// POST /v2/comments/{comment_id}/vote
#[utoipa::path(post, path = "/v2/comments/{commentId}/vote", tag = "comments", responses(
    (status = 200, description = "Success"),
    (status = 400, description = "Bad request", body = ErrorResponseTemplate),
    (status = 401, description = "Unauthorized", body = ErrorResponseTemplate),
    (status = 404, description = "Not found", body = ErrorResponseTemplate),
    (status = 500, description = "Internal error", body = ErrorResponseTemplate),
))]
pub async fn vote_comment(
    user: AuthUser,
    Path(comment_id): Path<Uuid>,
    State(db): State<DbPool>,
    Json(body): Json<VoteBody>,
) -> Result<Json<SuccessResponse<serde_json::Value>>, ApiError> {
    if body.value < -1 || body.value > 1 {
        return Err(ApiError::bad_request("Value must be -1, 0, or 1"));
    }

    // Check comment exists and not deleted
    let comment_exists: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM public.comments WHERE id = $1 AND deleted = FALSE)",
    )
    .bind(comment_id)
    .fetch_one(&db)
    .await?;

    if !comment_exists {
        return Err(ApiError::not_found("Comment not found"));
    }

    // Get previous vote
    let prev_value: Option<i16> = sqlx::query_scalar(
        "SELECT value FROM public.comment_votes WHERE comment_id = $1 AND user_id = $2",
    )
    .bind(comment_id)
    .bind(&user.id)
    .fetch_optional(&db)
    .await?
    .flatten();

    if body.value == 0 {
        // Remove vote
        sqlx::query("DELETE FROM public.comment_votes WHERE comment_id = $1 AND user_id = $2")
            .bind(comment_id)
            .bind(&user.id)
            .execute(&db)
            .await?;

        if let Some(old) = prev_value {
            if old > 0 {
                sqlx::query(
                    "UPDATE public.comments SET upvotes = GREATEST(upvotes - 1, 0) WHERE id = $1",
                )
                .bind(comment_id)
                .execute(&db)
                .await?;
            } else {
                sqlx::query("UPDATE public.comments SET downvotes = GREATEST(downvotes - 1, 0) WHERE id = $1")
                    .bind(comment_id).execute(&db).await?;
            }
        }
    } else {
        // Upsert vote
        sqlx::query(
            "INSERT INTO public.comment_votes (comment_id, user_id, value) VALUES ($1, $2, $3) \
             ON CONFLICT (comment_id, user_id) DO UPDATE SET value = $3",
        )
        .bind(comment_id)
        .bind(&user.id)
        .bind(body.value)
        .execute(&db)
        .await?;

        // Adjust counters
        if let Some(old) = prev_value {
            if old > 0 {
                sqlx::query(
                    "UPDATE public.comments SET upvotes = GREATEST(upvotes - 1, 0) WHERE id = $1",
                )
                .bind(comment_id)
                .execute(&db)
                .await?;
            } else {
                sqlx::query("UPDATE public.comments SET downvotes = GREATEST(downvotes - 1, 0) WHERE id = $1")
                    .bind(comment_id).execute(&db).await?;
            }
        }

        if body.value > 0 {
            sqlx::query("UPDATE public.comments SET upvotes = upvotes + 1 WHERE id = $1")
                .bind(comment_id)
                .execute(&db)
                .await?;
        } else {
            sqlx::query("UPDATE public.comments SET downvotes = downvotes + 1 WHERE id = $1")
                .bind(comment_id)
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

/// POST /v2/comments/{comment_id}/report
#[utoipa::path(post, path = "/v2/comments/{commentId}/report", tag = "comments", responses(
    (status = 200, description = "Success"),
    (status = 400, description = "Bad request", body = ErrorResponseTemplate),
    (status = 401, description = "Unauthorized", body = ErrorResponseTemplate),
    (status = 404, description = "Not found", body = ErrorResponseTemplate),
    (status = 500, description = "Internal error", body = ErrorResponseTemplate),
))]
pub async fn report_comment(
    user: AuthUser,
    Path(comment_id): Path<Uuid>,
    State(db): State<DbPool>,
    Json(body): Json<ReportBody>,
) -> Result<Json<SuccessResponse<serde_json::Value>>, ApiError> {
    let comment: Option<(String,)> =
        sqlx::query_as("SELECT user_id FROM public.comments WHERE id = $1 AND deleted = FALSE")
            .bind(comment_id)
            .fetch_optional(&db)
            .await?;

    let (author_id,) = comment.ok_or(ApiError::not_found("Comment not found"))?;

    if author_id == user.id {
        return Err(ApiError::bad_request("Cannot report your own comment"));
    }

    let exists: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM public.comment_reports WHERE comment_id = $1 AND user_id = $2)",
    )
    .bind(comment_id)
    .bind(&user.id)
    .fetch_one(&db)
    .await?;

    if exists {
        return Err(ApiError::bad_request("Already reported this comment"));
    }

    sqlx::query(
        "INSERT INTO public.comment_reports (comment_id, user_id, reason, description) VALUES ($1, $2, $3, $4)",
    )
    .bind(comment_id)
    .bind(&user.id)
    .bind(&body.reason)
    .bind(&body.description)
    .execute(&db)
    .await?;

    Ok(Json(SuccessResponse {
        result: "Success".to_string(),
        status: 201,
        data: serde_json::json!({}),
    }))
}

/// GET /v2/comments/{id}/votes
#[utoipa::path(get, path = "/v2/comments/{id}/votes", tag = "comments", responses(
    (status = 200, description = "Success", body = SuccessResponse<ItemsResponse<CommentVoteResponse>>),
    (status = 401, description = "Unauthorized", body = ErrorResponseTemplate),
    (status = 500, description = "Internal error", body = ErrorResponseTemplate),
))]
pub async fn get_votes(
    user: AuthUser,
    Path(target_id): Path<Uuid>,
    State(db): State<DbPool>,
) -> Result<Json<SuccessResponse<ItemsResponse<CommentVoteResponse>>>, ApiError> {
    #[derive(Debug, sqlx::FromRow)]
    struct VoteRow {
        comment_id: Uuid,
        value: i16,
    }

    let rows: Vec<VoteRow> = sqlx::query_as::<_, VoteRow>(
        "SELECT cv.comment_id, cv.value \
         FROM public.comment_votes cv \
         JOIN public.comments c ON c.id = cv.comment_id \
         WHERE c.target_id = $1 AND cv.user_id = $2",
    )
    .bind(target_id)
    .bind(&user.id)
    .fetch_all(&db)
    .await?;

    let items: Vec<CommentVoteResponse> = rows
        .into_iter()
        .map(|r| CommentVoteResponse {
            comment_id: r.comment_id,
            value: r.value,
            target_id,
        })
        .collect();

    Ok(Json(SuccessResponse {
        result: "Success".to_string(),
        status: 200,
        data: ItemsResponse { items },
    }))
}
