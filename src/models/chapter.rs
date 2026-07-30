use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

use super::manga_type::WorkFormat;

#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ChapterOption {
    pub label: String,
    pub value: String,
    pub scanlator_id: i32,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ChapterNavigation {
    pub number: f64,
    pub scanlator_id: i32,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ChapterResponse {
    pub id: Uuid,
    #[serde(rename = "type")]
    pub manga_type: WorkFormat,
    pub pages: i32,
    pub title: String,
    pub images: Vec<String>,
    pub number: f64,
    pub chapters: Vec<MangaChapter>,
    pub scanlator: Option<Scanlator>,
    #[serde(rename = "mangaId")]
    pub work_id: Uuid,
    #[serde(rename = "mangaTitle")]
    pub work_title: String,
    pub last_chapter: Option<ChapterNavigation>,
    pub next_chapter: Option<ChapterNavigation>,
    pub trackers: Vec<super::work::TrackerItem>,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct MangaChapter {
    pub id: Uuid,
    pub title: String,
    pub number: f64,
    pub scanlator_id: i32,
    pub pages: Option<i32>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct Scanlator {
    pub id: i32,
    pub name: String,
}
