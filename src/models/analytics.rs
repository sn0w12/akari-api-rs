use chrono::{DateTime, Utc};
use serde::Serialize;
use utoipa::ToSchema;

#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct AnalyticsOverviewResponse {
    #[schema(example = 120000)]
    pub total_requests: i64,
    #[schema(example = 8420)]
    pub unique_visitors: i64,
    #[schema(example = 38.5)]
    pub avg_response_time: f64,
    #[schema(example = 142.2)]
    pub p95_response_time: f64,
    #[schema(example = 210)]
    pub error_count: i64,
    #[schema(example = 0.0018)]
    pub error_rate: f64,
    #[schema(example = 114000)]
    pub status_2xx: i64,
    #[schema(example = 2100)]
    pub status_3xx: i64,
    #[schema(example = 3600)]
    pub status_4xx: i64,
    #[schema(example = 210)]
    pub status_5xx: i64,
}

#[derive(Debug, sqlx::FromRow, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct AnalyticsTimeseriesPoint {
    pub time: DateTime<Utc>,
    #[schema(example = 4150)]
    pub requests: i64,
    #[schema(example = 12)]
    pub errors: i64,
    #[schema(example = 940)]
    pub unique_visitors: i64,
    #[schema(example = 41.2)]
    pub avg_response_time: f64,
    #[schema(example = 21.3)]
    pub p50_response_time: f64,
    #[schema(example = 150.7)]
    pub p95_response_time: f64,
    #[schema(example = 412.9)]
    pub p99_response_time: f64,
    #[schema(example = 3890)]
    pub status_2xx: i64,
    #[schema(example = 120)]
    pub status_3xx: i64,
    #[schema(example = 210)]
    pub status_4xx: i64,
    #[schema(example = 12)]
    pub status_5xx: i64,
}

#[derive(Debug, sqlx::FromRow, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct AnalyticsTopItem {
    pub name: String,
    #[schema(example = 15320)]
    pub count: i64,
    #[schema(example = 35.2)]
    pub avg_response_time: f64,
}

#[derive(Debug, sqlx::FromRow, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct AnalyticsSlowestRoute {
    pub route: String,
    #[schema(example = 2140)]
    pub count: i64,
    #[schema(example = 183.4)]
    pub avg_response_time: f64,
    #[schema(example = 1240)]
    pub max_response_time: i64,
    #[schema(example = 42)]
    pub error_count: i64,
}

#[derive(Debug, sqlx::FromRow, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct AnalyticsRequestRow {
    pub id: i32,
    pub created_at: Option<DateTime<Utc>>,
    pub hostname: String,
    pub ip_address: String,
    pub user_agent: String,
    pub path: String,
    pub method: String,
    pub response_time: i32,
    pub status: i32,
    pub route: String,
    pub country_code: String,
}
