use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct UserListResponse {
    pub id: Uuid,
    #[serde(rename = "userId")]
    pub user_id: String,
    pub title: String,
    pub description: Option<String>,
    #[serde(rename = "isPublic")]
    pub is_public: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    #[serde(rename = "totalEntries")]
    pub total_entries: i32,
}

#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ListEntryResponse {
    pub id: Uuid,
    #[serde(rename = "listId")]
    pub list_id: Uuid,
    #[serde(rename = "mangaId")]
    pub work_id: Uuid,
    #[serde(rename = "orderIndex")]
    pub order_index: i32,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    #[serde(rename = "mangaTitle")]
    pub manga_title: String,
    #[serde(rename = "mangaCover")]
    pub manga_cover: String,
    #[serde(rename = "mangaDescription")]
    pub manga_description: String,
}

#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct UserListDetailResponse {
    pub id: Uuid,
    #[serde(rename = "userId")]
    pub user_id: String,
    pub title: String,
    pub description: Option<String>,
    #[serde(rename = "isPublic")]
    pub is_public: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    #[serde(rename = "totalEntries")]
    pub total_entries: i32,
    pub entries: Vec<ListEntryResponse>,
    pub user: crate::models::user::UserResponse,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct CreateListBody {
    pub title: String,
    pub description: Option<String>,
    #[serde(rename = "isPublic")]
    pub is_public: Option<bool>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct UpdateEntryBody {
    #[serde(rename = "newOrderIndex")]
    pub order_index: Option<i32>,
}
