use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub enum CommentSortOrder {
    Latest,
    Oldest,
    Upvoted,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct UserProfile {
    pub id: String,
    pub username: String,
    #[serde(rename = "displayName")]
    pub display_name: String,
    pub role: String,
    pub banned: bool,
}

#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct CommentResponse {
    pub id: Uuid,
    #[serde(rename = "targetType")]
    pub target_type: String,
    #[serde(rename = "targetId")]
    pub target_id: Uuid,
    #[serde(rename = "userProfile")]
    pub user_profile: UserProfile,
    #[serde(rename = "parentId")]
    pub parent_id: Option<Uuid>,
    pub content: String,
    #[serde(rename = "createdAt")]
    pub created_at: DateTime<Utc>,
    #[serde(rename = "updatedAt")]
    pub updated_at: DateTime<Utc>,
    pub edited: bool,
    pub deleted: bool,
    pub upvotes: i32,
    pub downvotes: i32,
    #[serde(rename = "replyCount")]
    pub reply_count: Option<i64>,
}

#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct PaginatedCommentResponse {
    pub items: Vec<CommentResponse>,
    pub total_items: i64,
    pub current_page: i32,
    pub page_size: i32,
    pub total_pages: i32,
}

#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct CommentVoteResponse {
    #[serde(rename = "commentId")]
    pub comment_id: Uuid,
    pub value: i16,
    #[serde(rename = "targetId")]
    pub target_id: Uuid,
}

#[derive(Debug, Serialize, Deserialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct CommentWithRepliesResponse {
    pub id: Uuid,
    #[serde(rename = "targetType")]
    pub target_type: String,
    #[serde(rename = "targetId")]
    pub target_id: Uuid,
    #[serde(rename = "userProfile")]
    pub user_profile: UserProfile,
    #[serde(rename = "parentId")]
    pub parent_id: Option<Uuid>,
    pub content: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub edited: bool,
    pub deleted: bool,
    pub upvotes: i32,
    pub downvotes: i32,
    #[schema(value_type = Vec<CommentResponse>)]
    pub replies: Vec<CommentWithRepliesResponse>,
}

#[derive(Debug, Serialize, Deserialize, utoipa::ToSchema)]
pub enum CommentReportReason {
    Spam,
    Harassment,
    Inappropriate,
    HateSpeech,
    Other,
}
