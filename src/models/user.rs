use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub enum UserRole {
    #[serde(rename = "user")]
    User,
    #[serde(rename = "admin")]
    Admin,
    #[serde(rename = "owner")]
    Owner,
}

#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct UserResponse {
    #[serde(rename = "userId")]
    pub user_id: String,
    pub username: String,
    #[serde(rename = "displayName")]
    pub display_name: String,
    pub role: UserRole,
    pub banned: bool,
}

#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct UserProfileDetailsResponse {
    #[serde(rename = "userId")]
    pub user_id: String,
    pub username: String,
    #[serde(rename = "displayName")]
    pub display_name: String,
    pub role: UserRole,
    pub banned: bool,
    pub created_at: Option<DateTime<Utc>>,
    pub total_comments: Option<i64>,
    pub total_upvotes: Option<i64>,
    pub total_downvotes: Option<i64>,
    pub total_bookmarks: Option<i64>,
    pub total_uploads: Option<i64>,
    pub total_lists: Option<i64>,
}

#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct UserListResponse {
    pub items: Vec<UserProfileDetailsResponse>,
    pub total_items: i64,
    pub current_page: i32,
    pub page_size: i32,
    pub total_pages: i32,
}
