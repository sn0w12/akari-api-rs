use axum::Json;
use axum::extract::{Query, State};
use chrono::{DateTime, Duration, Utc};
use serde::Deserialize;
use sqlx::{Postgres, QueryBuilder};
use utoipa::IntoParams;

use crate::auth::{AdminAuthUser, AuthUser};
use crate::db::DbPool;
use crate::error::{ApiError, ErrorResponseTemplate};
use crate::models::analytics::{
    AnalyticsOverviewResponse, AnalyticsRequestRow, AnalyticsSlowestRoute, AnalyticsTimeseriesPoint,
    AnalyticsTopItem,
};
use crate::response::{PaginatedResponse, SuccessResponse};

#[derive(Debug, Clone, Copy, Deserialize, utoipa::ToSchema)]
#[serde(rename_all = "lowercase")]
pub enum AnalyticsInterval {
    Hour,
    Day,
}

#[derive(Debug, Clone, Deserialize, IntoParams)]
#[serde(rename_all = "camelCase")]
pub struct AnalyticsRangeParams {
    /// Inclusive start of the range (ISO 8601). Defaults to 30 days ago.
    pub from: Option<DateTime<Utc>>,
    /// Exclusive end of the range (ISO 8601). Defaults to now.
    pub to: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Deserialize, IntoParams)]
#[serde(rename_all = "camelCase")]
pub struct AnalyticsTimeseriesParams {
    /// Inclusive start of the range (ISO 8601). Defaults to 30 days ago.
    pub from: Option<DateTime<Utc>>,
    /// Exclusive end of the range (ISO 8601). Defaults to now.
    pub to: Option<DateTime<Utc>>,
    /// Bucket granularity. Defaults to "day".
    pub interval: Option<AnalyticsInterval>,
}

#[derive(Debug, Clone, Deserialize, IntoParams)]
#[serde(rename_all = "camelCase")]
pub struct AnalyticsTopParams {
    /// Inclusive start of the range (ISO 8601). Defaults to 30 days ago.
    pub from: Option<DateTime<Utc>>,
    /// Exclusive end of the range (ISO 8601). Defaults to now.
    pub to: Option<DateTime<Utc>>,
    /// Dimension to group by: route, path, method, status, countryCode, userAgent, ipAddress or hostname. Defaults to "route".
    pub dimension: Option<String>,
    /// Max number of results. Defaults to 10.
    pub limit: Option<i32>,
}

#[derive(Debug, Clone, Deserialize, IntoParams)]
#[serde(rename_all = "camelCase")]
pub struct AnalyticsRequestsParams {
    /// Inclusive start of the range (ISO 8601). Defaults to 7 days ago.
    pub from: Option<DateTime<Utc>>,
    /// Exclusive end of the range (ISO 8601). Defaults to now.
    pub to: Option<DateTime<Utc>>,
    pub page: Option<i32>,
    #[serde(rename = "pageSize")]
    pub page_size: Option<i32>,
    pub method: Option<String>,
    pub status: Option<i32>,
    /// Substring match against the request path.
    pub path: Option<String>,
    pub route: Option<String>,
    pub hostname: Option<String>,
}

fn range(params: &AnalyticsRangeParams) -> (DateTime<Utc>, DateTime<Utc>) {
    (
        params.from.unwrap_or(Utc::now() - Duration::days(30)),
        params.to.unwrap_or(Utc::now()),
    )
}

fn pagination(page: Option<i32>, page_size: Option<i32>) -> (i32, i32, i64) {
    let p = page.unwrap_or(1).max(1);
    let ps = page_size.unwrap_or(20).clamp(1, 100);
    (p, ps, ((p - 1) as i64) * (ps as i64))
}

fn ensure_admin(user: &AuthUser) -> Result<(), ApiError> {
    let role = user.role.as_deref().unwrap_or("user");
    if role != "admin" && role != "owner" {
        return Err(ApiError::Forbidden {
            message: "Admin access required".into(),
        });
    }
    Ok(())
}

/// GET /v2/analytics/overview
#[utoipa::path(get, path = "/v2/analytics/overview", tag = "analytics", params(AnalyticsRangeParams), responses(
    (status = 200, description = "Success", body = SuccessResponse<AnalyticsOverviewResponse>),
    (status = 401, description = "Unauthorized", body = ErrorResponseTemplate),
    (status = 403, description = "Forbidden", body = ErrorResponseTemplate),
    (status = 500, description = "Internal error", body = ErrorResponseTemplate),
))]
pub async fn overview(
    user: AdminAuthUser,
    Query(params): Query<AnalyticsRangeParams>,
    State(db): State<DbPool>,
) -> Result<Json<SuccessResponse<AnalyticsOverviewResponse>>, ApiError> {
    ensure_admin(&user.0)?;
    let (from, to) = range(&params);

    let row = sqlx::query_as::<_, AnalyticsOverviewRow>(
        "SELECT \
            COUNT(*)::bigint AS total_requests, \
            COUNT(DISTINCT ip_address)::bigint AS unique_visitors, \
            COALESCE(AVG(response_time)::float8, 0) AS avg_response_time, \
            COALESCE(percentile_cont(0.95) WITHIN GROUP (ORDER BY response_time), 0) AS p95_response_time, \
            COUNT(*) FILTER (WHERE status >= 500)::bigint AS error_count, \
            COUNT(*) FILTER (WHERE status >= 200 AND status < 300)::bigint AS status_2xx, \
            COUNT(*) FILTER (WHERE status >= 300 AND status < 400)::bigint AS status_3xx, \
            COUNT(*) FILTER (WHERE status >= 400 AND status < 500)::bigint AS status_4xx, \
            COUNT(*) FILTER (WHERE status >= 500)::bigint AS status_5xx \
         FROM analytics.requests \
         WHERE created_at >= $1 AND created_at < $2",
    )
    .bind(from)
    .bind(to)
    .fetch_one(&db)
    .await?;

    let error_rate = if row.total_requests > 0 {
        row.error_count as f64 / row.total_requests as f64
    } else {
        0.0
    };

    Ok(Json(SuccessResponse::new(AnalyticsOverviewResponse {
        total_requests: row.total_requests,
        unique_visitors: row.unique_visitors,
        avg_response_time: row.avg_response_time,
        p95_response_time: row.p95_response_time,
        error_count: row.error_count,
        error_rate,
        status_2xx: row.status_2xx,
        status_3xx: row.status_3xx,
        status_4xx: row.status_4xx,
        status_5xx: row.status_5xx,
    })))
}

#[derive(Debug, sqlx::FromRow)]
struct AnalyticsOverviewRow {
    total_requests: i64,
    unique_visitors: i64,
    avg_response_time: f64,
    p95_response_time: f64,
    error_count: i64,
    status_2xx: i64,
    status_3xx: i64,
    status_4xx: i64,
    status_5xx: i64,
}

/// GET /v2/analytics/timeseries
#[utoipa::path(get, path = "/v2/analytics/timeseries", tag = "analytics", params(AnalyticsTimeseriesParams), responses(
    (status = 200, description = "Success", body = SuccessResponse<Vec<AnalyticsTimeseriesPoint>>),
    (status = 401, description = "Unauthorized", body = ErrorResponseTemplate),
    (status = 403, description = "Forbidden", body = ErrorResponseTemplate),
    (status = 500, description = "Internal error", body = ErrorResponseTemplate),
))]
pub async fn timeseries(
    user: AdminAuthUser,
    Query(params): Query<AnalyticsTimeseriesParams>,
    State(db): State<DbPool>,
) -> Result<Json<SuccessResponse<Vec<AnalyticsTimeseriesPoint>>>, ApiError> {
    ensure_admin(&user.0)?;
    let (from, to) = range(&AnalyticsRangeParams {
        from: params.from,
        to: params.to,
    });

    let bucket = match params.interval.unwrap_or(AnalyticsInterval::Day) {
        AnalyticsInterval::Hour => "hour",
        AnalyticsInterval::Day => "day",
    };

    let rows: Vec<AnalyticsTimeseriesPoint> = sqlx::query_as(
        "SELECT \
            date_trunc($3, created_at) AS time, \
            COUNT(*)::bigint AS requests, \
            COUNT(*) FILTER (WHERE status >= 500)::bigint AS errors, \
            COUNT(DISTINCT ip_address)::bigint AS unique_visitors, \
            COALESCE(AVG(response_time)::float8, 0) AS avg_response_time, \
            COALESCE(percentile_cont(0.50) WITHIN GROUP (ORDER BY response_time), 0) AS p50_response_time, \
            COALESCE(percentile_cont(0.95) WITHIN GROUP (ORDER BY response_time), 0) AS p95_response_time, \
            COALESCE(percentile_cont(0.99) WITHIN GROUP (ORDER BY response_time), 0) AS p99_response_time, \
            COUNT(*) FILTER (WHERE status >= 200 AND status < 300)::bigint AS status_2xx, \
            COUNT(*) FILTER (WHERE status >= 300 AND status < 400)::bigint AS status_3xx, \
            COUNT(*) FILTER (WHERE status >= 400 AND status < 500)::bigint AS status_4xx, \
            COUNT(*) FILTER (WHERE status >= 500)::bigint AS status_5xx \
         FROM analytics.requests \
         WHERE created_at >= $1 AND created_at < $2 \
         GROUP BY 1 ORDER BY 1 ASC",
    )
    .bind(from)
    .bind(to)
    .bind(bucket)
    .fetch_all(&db)
    .await?;

    Ok(Json(SuccessResponse::new(rows)))
}

/// GET /v2/analytics/top
#[utoipa::path(get, path = "/v2/analytics/top", tag = "analytics", params(AnalyticsTopParams), responses(
    (status = 200, description = "Success", body = SuccessResponse<Vec<AnalyticsTopItem>>),
    (status = 401, description = "Unauthorized", body = ErrorResponseTemplate),
    (status = 403, description = "Forbidden", body = ErrorResponseTemplate),
    (status = 500, description = "Internal error", body = ErrorResponseTemplate),
))]
pub async fn top(
    user: AdminAuthUser,
    Query(params): Query<AnalyticsTopParams>,
    State(db): State<DbPool>,
) -> Result<Json<SuccessResponse<Vec<AnalyticsTopItem>>>, ApiError> {
    ensure_admin(&user.0)?;
    let (from, to) = range(&AnalyticsRangeParams {
        from: params.from,
        to: params.to,
    });

    let limit = params.limit.unwrap_or(10).clamp(1, 50);
    let column = match params.dimension.as_deref().unwrap_or("route") {
        "path" => "path",
        "method" => "method",
        "status" => "status::text",
        "countryCode" | "country_code" => "country_code",
        "userAgent" | "user_agent" => "user_agent",
        "ipAddress" | "ip_address" => "ip_address",
        "hostname" => "hostname",
        _ => "route",
    };

    let mut builder = QueryBuilder::new("SELECT ");
    builder.push(column);
    builder.push(
        " AS name, COUNT(*)::bigint AS count, COALESCE(AVG(response_time)::float8, 0) AS avg_response_time \
         FROM analytics.requests \
         WHERE created_at >= ",
    );
    builder.push_bind(from);
    builder.push(" AND created_at < ");
    builder.push_bind(to);
    builder.push(" GROUP BY name ORDER BY count DESC LIMIT ");
    builder.push_bind(limit);

    let rows: Vec<AnalyticsTopItem> = builder.build_query_as().fetch_all(&db).await?;

    Ok(Json(SuccessResponse::new(rows)))
}

/// GET /v2/analytics/slowest
#[utoipa::path(get, path = "/v2/analytics/slowest", tag = "analytics", params(AnalyticsTopParams), responses(
    (status = 200, description = "Success", body = SuccessResponse<Vec<AnalyticsSlowestRoute>>),
    (status = 401, description = "Unauthorized", body = ErrorResponseTemplate),
    (status = 403, description = "Forbidden", body = ErrorResponseTemplate),
    (status = 500, description = "Internal error", body = ErrorResponseTemplate),
))]
pub async fn slowest(
    user: AdminAuthUser,
    Query(params): Query<AnalyticsTopParams>,
    State(db): State<DbPool>,
) -> Result<Json<SuccessResponse<Vec<AnalyticsSlowestRoute>>>, ApiError> {
    ensure_admin(&user.0)?;
    let (from, to) = range(&AnalyticsRangeParams {
        from: params.from,
        to: params.to,
    });

    let limit = params.limit.unwrap_or(10).clamp(1, 50);

    let rows: Vec<AnalyticsSlowestRoute> = sqlx::query_as(
        "SELECT route, \
            COUNT(*)::bigint AS count, \
            COALESCE(AVG(response_time)::float8, 0) AS avg_response_time, \
            MAX(response_time)::bigint AS max_response_time, \
            COUNT(*) FILTER (WHERE status >= 500)::bigint AS error_count \
         FROM analytics.requests \
         WHERE created_at >= $1 AND created_at < $2 \
         GROUP BY route ORDER BY avg_response_time DESC LIMIT $3",
    )
    .bind(from)
    .bind(to)
    .bind(limit)
    .fetch_all(&db)
    .await?;

    Ok(Json(SuccessResponse::new(rows)))
}

/// GET /v2/analytics/requests
#[utoipa::path(get, path = "/v2/analytics/requests", tag = "analytics", params(AnalyticsRequestsParams), responses(
    (status = 200, description = "Success", body = PaginatedResponse<AnalyticsRequestRow>),
    (status = 401, description = "Unauthorized", body = ErrorResponseTemplate),
    (status = 403, description = "Forbidden", body = ErrorResponseTemplate),
    (status = 500, description = "Internal error", body = ErrorResponseTemplate),
))]
pub async fn requests(
    user: AdminAuthUser,
    Query(params): Query<AnalyticsRequestsParams>,
    State(db): State<DbPool>,
) -> Result<Json<PaginatedResponse<AnalyticsRequestRow>>, ApiError> {
    ensure_admin(&user.0)?;
    let (from, to) = range(&AnalyticsRangeParams {
        from: params.from,
        to: params.to,
    });
    let (page, page_size, offset) = pagination(params.page, params.page_size);

    let mut count_builder = QueryBuilder::new("SELECT COUNT(*)::bigint FROM analytics.requests");
    push_filters(&mut count_builder, from, to, &params);
    let total: i64 = count_builder.build_query_scalar().fetch_one(&db).await?;

    let mut data_builder = QueryBuilder::new(
        "SELECT id, created_at, hostname, ip_address, user_agent, path, method, response_time, status, route, country_code \
         FROM analytics.requests",
    );
    push_filters(&mut data_builder, from, to, &params);
    data_builder.push(" ORDER BY created_at DESC NULLS LAST LIMIT ");
    data_builder.push_bind(page_size);
    data_builder.push(" OFFSET ");
    data_builder.push_bind(offset);

    let items: Vec<AnalyticsRequestRow> = data_builder.build_query_as().fetch_all(&db).await?;
    let total_pages = ((total + page_size as i64 - 1) / page_size as i64) as i32;

    Ok(Json(PaginatedResponse {
        items,
        total_items: total,
        current_page: page,
        page_size,
        total_pages,
    }))
}

fn push_filters(
    builder: &mut QueryBuilder<Postgres>,
    from: DateTime<Utc>,
    to: DateTime<Utc>,
    params: &AnalyticsRequestsParams,
) {
    builder.push(" WHERE created_at >= ");
    builder.push_bind(from);
    builder.push(" AND created_at < ");
    builder.push_bind(to);

    if let Some(method) = &params.method {
        if !method.is_empty() {
            builder.push(" AND method = ");
            builder.push_bind(method);
        }
    }
    if let Some(status) = params.status {
        builder.push(" AND status = ");
        builder.push_bind(status);
    }
    if let Some(path) = &params.path {
        if !path.is_empty() {
            builder.push(" AND path ILIKE ");
            builder.push_bind(format!("%{}%", path));
        }
    }
    if let Some(route) = &params.route {
        if !route.is_empty() {
            builder.push(" AND route = ");
            builder.push_bind(route);
        }
    }
    if let Some(hostname) = &params.hostname {
        if !hostname.is_empty() {
            builder.push(" AND hostname = ");
            builder.push_bind(hostname);
        }
    }
}
