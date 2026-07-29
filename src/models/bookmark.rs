use super::chapter::MangaChapter;
use super::manga_type::WorkFormat;
use super::work::TrackerItem;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub enum HistoryBucket {
    Hour,
    Day,
    Week,
    Month,
    Year,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct DayOfWeekReadCount {
    #[serde(rename = "dayOfWeek")]
    pub day_of_week: i32,
    pub count: i64,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct HourReadCount {
    pub hour: i32,
    pub count: i64,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ReadingHistoryTimelineEntry {
    pub date: String,
    pub reads: i64,
}

#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct BookmarkResponse {
    #[serde(rename = "bookmarkId")]
    pub bookmark_id: Uuid,
    #[serde(rename = "bookmarkCreatedAt")]
    pub bookmark_created_at: DateTime<Utc>,
    #[serde(rename = "bookmarkUpdatedAt")]
    pub bookmark_updated_at: DateTime<Utc>,
    #[serde(rename = "mangaId")]
    pub work_id: Uuid,
    pub title: String,
    pub cover: String,
    pub description: String,
    pub status: String,
    #[serde(rename = "type")]
    pub manga_type: WorkFormat,
    pub authors: Vec<String>,
    pub genres: Vec<String>,
    pub views: i32,
    pub score: f64,
    pub trackers: Vec<TrackerItem>,
    #[serde(default)]
    pub alternative_titles: Vec<super::work::AlternativeTitle>,
    #[serde(rename = "mangaCreatedAt")]
    pub work_created_at: DateTime<Utc>,
    #[serde(rename = "mangaUpdatedAt")]
    pub work_updated_at: DateTime<Utc>,
    pub last_read_chapter: MangaChapter,
    pub latest_chapter: MangaChapter,
    #[serde(default)]
    pub next_chapter: MangaChapter,
    pub chapters_behind: i32,
}

#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct PaginatedBookmarkResponse {
    pub items: Vec<BookmarkResponse>,
    pub total_items: i64,
    pub current_page: i32,
    pub page_size: i32,
    pub total_pages: i32,
}

#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct BookmarkDetailResponse {
    pub id: Uuid,
    pub title: Option<String>,
    pub number: f64,
    pub pages: Option<i16>,
    pub scanlator_id: Option<i32>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct BookmarkBatchItem {
    #[serde(rename = "mangaId")]
    pub work_id: Uuid,
    #[serde(rename = "chapterNumber")]
    pub chapter_number: Option<f64>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct BookmarkBatchBody {
    pub items: Vec<BookmarkBatchItem>,
}

#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ReadingHistoryResponse {
    pub date: String,
    pub reads: i64,
}

#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ReadingStatsResponse {
    pub total_reads: i64,
    pub unique_manga: i64,
    pub avg_per_day: f64,
    pub current_streak: i64,
    pub longest_streak: i64,
    pub top_genres: Vec<GenreCount>,
    #[serde(rename = "readsByDayOfWeek")]
    pub reads_by_day_of_week: Vec<DayOfWeekReadCount>,
    #[serde(rename = "readsByHour")]
    pub reads_by_hour: Vec<HourReadCount>,
}

#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct GenreCount {
    pub name: String,
    pub count: i64,
}
