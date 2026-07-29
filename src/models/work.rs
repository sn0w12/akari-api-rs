use chrono::{DateTime, Utc};
use serde::Serialize;
use utoipa::ToSchema;
use uuid::Uuid;

use super::manga_type::WorkFormat;

#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct MangaRatingDistribution {
    #[serde(rename = "1")] pub score1: i32,
    #[serde(rename = "2")] pub score2: i32,
    #[serde(rename = "3")] pub score3: i32,
    #[serde(rename = "4")] pub score4: i32,
    #[serde(rename = "5")] pub score5: i32,
    #[serde(rename = "6")] pub score6: i32,
    #[serde(rename = "7")] pub score7: i32,
    #[serde(rename = "8")] pub score8: i32,
    #[serde(rename = "9")] pub score9: i32,
    #[serde(rename = "10")] pub score10: i32,
}

#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct MangaRatingResponse {
    pub average: f64, pub total: i32, pub distribution: MangaRatingDistribution,
}

#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct RatingResponse {
    pub rating: Option<i32>,
}

#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct MangaResponse {
    pub id: Uuid, pub title: String, pub cover: String,
    pub description: String, pub status: String,
    #[serde(rename = "type")] pub manga_type: WorkFormat,
    pub authors: Vec<String>, pub genres: Vec<String>,
    pub views: i32, pub rating: MangaRatingResponse,
    #[serde(default)] pub alternative_titles: Vec<String>,
    pub mal_id: Option<i32>, pub ani_id: Option<i32>,
    pub preferred_scanlator_id: Option<i32>,
    pub created_at: DateTime<Utc>, pub updated_at: DateTime<Utc>,
}

/// MangaDetailResponse extends MangaResponse (all manga fields) + chapters
#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct MangaDetailResponse {
    pub id: Uuid, pub title: String, pub cover: String,
    pub description: String, pub status: String,
    #[serde(rename = "type")] pub manga_type: WorkFormat,
    pub authors: Vec<String>, pub genres: Vec<String>,
    pub views: i32, pub rating: MangaRatingResponse,
    #[serde(default)] pub alternative_titles: Vec<String>,
    pub mal_id: Option<i32>, pub ani_id: Option<i32>,
    pub preferred_scanlator_id: Option<i32>,
    pub created_at: DateTime<Utc>, pub updated_at: DateTime<Utc>,
    pub chapters: Vec<super::chapter::MangaChapter>,
}

#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct MangaChapterResponse {
    pub scanlators: Vec<super::chapter::Scanlator>,
    pub chapters: Vec<super::chapter::MangaChapter>,
    pub preferred_scanlator_id: Option<i32>,
}

#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct MangaSearchResponse {
    pub id: Uuid, pub title: String, pub cover: String,
    pub description: String, pub status: String,
    #[serde(rename = "type")] pub manga_type: WorkFormat,
    pub authors: Vec<String>, pub genres: Vec<String>,
    pub views: i32, pub rating: MangaRatingResponse,
    #[serde(default)] pub alternative_titles: Vec<String>,
    pub mal_id: Option<i32>, pub ani_id: Option<i32>,
    pub preferred_scanlator_id: Option<i32>,
    pub created_at: DateTime<Utc>, pub updated_at: DateTime<Utc>,
    pub rank: f64,
}

#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct MangaIdsResponse {
    pub items: Vec<Uuid>,
    pub total_items: i64, pub current_page: i32,
    pub page_size: i32, pub total_pages: i32,
}

#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ChapterIdsResponse {
    #[serde(rename = "mangaId")] pub work_id: Uuid,
    pub chapter_ids: Vec<f64>,
}
