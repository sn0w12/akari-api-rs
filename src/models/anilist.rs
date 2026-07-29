use serde::Deserialize;
use utoipa::ToSchema;

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct AniViewer {
    pub id: i32,
    pub name: String,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct AniMediaListCollection {
    pub lists: Vec<AniList>,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct AniList {
    pub name: String,
    pub entries: Vec<AniEntry>,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct AniEntry {
    pub id: i64,
    pub score: i32,
    pub progress: i32,
    pub status: String,
    pub media: AniMedia,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct AniMedia {
    pub id: i32,
    pub title: AniTitle,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct AniTitle {
    pub english: Option<String>,
}

use serde::Serialize;

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct AniUpdatedEntry {
    pub id: i64,
    pub status: String,
    pub progress: i32,
}
