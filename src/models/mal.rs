use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct MalTokenResponse {
    #[serde(alias = "access_token")]
    pub access_token: String,
    #[serde(alias = "refresh_token")]
    pub refresh_token: String,
    #[serde(alias = "expires_in")]
    pub expires_in: i32,
    #[serde(alias = "token_type")]
    pub token_type: String,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct MalMangaListResponse {
    pub data: Vec<MalMangaListItem>,
    pub paging: MalPaging,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct MalMangaListItem {
    pub node: MalMangaNode,
    #[serde(alias = "list_status")]
    pub list_status: MalListStatus,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct MalMangaNode {
    pub id: i32,
    pub title: String,
    pub main_picture: Option<MalMainPicture>,
    #[serde(alias = "media_type")]
    pub media_type: String,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct MalMainPicture {
    pub medium: String,
    pub large: String,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct MalListStatus {
    pub status: String,
    #[serde(alias = "is_rereading")]
    pub is_rereading: bool,
    #[serde(alias = "num_volumes_read")]
    pub num_volumes_read: i32,
    #[serde(alias = "num_chapters_read")]
    pub num_chapters_read: i32,
    pub score: i32,
    #[serde(alias = "updated_at")]
    pub updated_at: String,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct MalPaging {
    pub next: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct MalUser {
    pub id: i32,
    pub name: String,
    pub location: String,
    #[serde(alias = "joined_at")]
    pub joined_at: String,
}
