use axum::Json;
use axum::extract::{Query, State};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use utoipa::ToSchema;

use crate::config::Config;
use crate::db::DbPool;
use crate::error::{ApiError, ErrorResponseTemplate};
use crate::models::mal::*;
use crate::response::SuccessResponse;

#[derive(Debug, Deserialize, utoipa::ToSchema, utoipa::IntoParams)]
pub struct MalTokenRequest {
    pub code: String,
    #[serde(rename = "codeVerifier")]
    pub code_verifier: String,
    #[serde(rename = "redirectUri")]
    pub redirect_uri: String,
}

#[derive(Debug, Deserialize, utoipa::ToSchema, utoipa::IntoParams)]
pub struct MalUpdateMangaListRequest {
    #[serde(rename = "mangaId")]
    pub manga_id: i32,
    #[serde(rename = "numChaptersRead")]
    pub num_chapters_read: i32,
}

#[derive(Debug, Deserialize, utoipa::IntoParams)]
pub struct MalMangaListQuery {
    pub status: Option<String>,
    pub sort: Option<String>,
    pub limit: Option<i32>,
    pub offset: Option<i32>,
}

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

/// Helper: set a cookie header value for the response in the format expected by Set-Cookie.
#[allow(dead_code)]
fn set_cookie(name: &str, value: &str, max_age_secs: Option<i64>) -> String {
    let mut cookie = format!("{}={}; Path=/; SameSite=Lax", name, value);
    if let Some(secs) = max_age_secs {
        cookie.push_str(&format!("; Max-Age={}", secs));
    }
    cookie
}

fn delete_cookie(name: &str) -> String {
    format!("{}=; Path=/; Max-Age=0", name)
}

/// POST /v2/mal/token
#[utoipa::path(post, path = "/v2/mal/token", tag = "mal", operation_id = "mal_token", responses(
    (status = 200, description = "Success", body = SuccessResponse<MalTokenResponse>),
    (status = 400, description = "Bad request", body = ErrorResponseTemplate),
    (status = 500, description = "Internal error", body = ErrorResponseTemplate),
))]
pub async fn token_exchange(
    State(_db): State<DbPool>,
    State(config): State<Config>,
    Json(body): Json<MalTokenRequest>,
) -> Result<Json<SuccessResponse<MalTokenResponse>>, ApiError> {
    if body.code.is_empty() || body.code_verifier.is_empty() {
        return Err(ApiError::bad_request("Missing input"));
    }

    let client = reqwest::Client::new();
    let params = [
        ("client_id", config.mal_client_id.as_str()),
        ("code", &body.code),
        ("code_verifier", &body.code_verifier),
        ("grant_type", "authorization_code"),
        ("redirect_uri", &body.redirect_uri),
    ];

    let resp = client
        .post("https://myanimelist.net/v1/oauth2/token")
        .form(&params)
        .send()
        .await
        .map_err(|e| ApiError::internal(format!("MAL request failed: {}", e)))?;

    let status = resp.status();
    let text = resp.text().await.unwrap_or_default();

    if status.is_success() {
        let data: MalTokenResponse = serde_json::from_str(&text)
            .map_err(|e| ApiError::internal(format!("MAL parse error: {}", e)))?;

        Ok(Json(SuccessResponse {
            result: "Success".to_string(),
            status: 200,
            data,
        }))
    } else {
        let err: Value = serde_json::from_str(&text).unwrap_or_default();
        let msg = err["message"].as_str().unwrap_or("MAL error").to_string();
        Err(ApiError::internal(msg))
    }
}

/// GET /v2/mal/mangalist
#[utoipa::path(get, path = "/v2/mal/mangalist", tag = "mal", operation_id = "mal_get_manga_list", params(MalMangaListQuery), responses(
    (status = 200, description = "Success", body = SuccessResponse<MalMangaListResponse>),
    (status = 400, description = "Bad request", body = ErrorResponseTemplate),
    (status = 500, description = "Internal error", body = ErrorResponseTemplate),
))]
pub async fn get_manga_list(
    cookies: axum_extra::extract::cookie::CookieJar,
    Query(params): Query<MalMangaListQuery>,
) -> Result<Json<SuccessResponse<MalMangaListResponse>>, ApiError> {
    let access_token = cookies
        .get("mal_access_token")
        .map(|c| c.value().to_string())
        .ok_or(ApiError::bad_request("Missing access token"))?;

    let limit = params.limit.unwrap_or(100).clamp(1, 1000);
    let offset = params.offset.unwrap_or(0).max(0);

    let mut query_params = vec![
        format!("limit={}", limit),
        format!("offset={}", offset),
        "nsfw=1".to_string(),
        "fields=list_status,media_type".to_string(),
    ];
    if let Some(ref status) = params.status {
        query_params.push(format!("status={}", status));
    }
    if let Some(ref sort) = params.sort {
        query_params.push(format!("sort={}", sort));
    }

    let url = format!(
        "https://api.myanimelist.net/v2/users/@me/mangalist?{}",
        query_params.join("&")
    );

    let client = reqwest::Client::new();
    let resp = client
        .get(&url)
        .bearer_auth(&access_token)
        .send()
        .await
        .map_err(|e| ApiError::internal(format!("MAL request failed: {}", e)))?;

    let status_code = resp.status();
    let text = resp.text().await.unwrap_or_default();

    if status_code.is_success() {
        let data: MalMangaListResponse = serde_json::from_str(&text)
            .map_err(|e| ApiError::internal(format!("MAL parse error: {}", e)))?;
        Ok(Json(SuccessResponse {
            result: "Success".to_string(),
            status: 200,
            data,
        }))
    } else {
        let msg = format!("MAL error: {}", status_code);
        Err(ApiError::internal(msg))
    }
}

/// POST /v2/mal/mangalist
#[utoipa::path(post, path = "/v2/mal/mangalist", tag = "mal", operation_id = "mal_update_manga_list", responses(
    (status = 200, description = "Success", body = SuccessResponse<MalListStatus>),
    (status = 400, description = "Bad request", body = ErrorResponseTemplate),
    (status = 500, description = "Internal error", body = ErrorResponseTemplate),
))]
pub async fn update_manga_list(
    cookies: axum_extra::extract::cookie::CookieJar,
    Json(body): Json<MalUpdateMangaListRequest>,
) -> Result<Json<SuccessResponse<MalListStatus>>, ApiError> {
    if body.manga_id <= 0 || body.num_chapters_read < 0 {
        return Err(ApiError::bad_request("Invalid input"));
    }

    let access_token = cookies
        .get("mal_access_token")
        .map(|c| c.value().to_string())
        .ok_or(ApiError::bad_request("Missing access token"))?;

    let client = reqwest::Client::new();
    let params = [("num_chapters_read", body.num_chapters_read.to_string())];
    let url = format!(
        "https://api.myanimelist.net/v2/manga/{}/my_list_status",
        body.manga_id
    );

    let resp = client
        .patch(&url)
        .bearer_auth(&access_token)
        .form(&params)
        .send()
        .await
        .map_err(|e| ApiError::internal(format!("MAL request failed: {}", e)))?;

    let status_code = resp.status();
    let text = resp.text().await.unwrap_or_default();

    if status_code.is_success() {
        let data: MalListStatus = serde_json::from_str(&text)
            .map_err(|e| ApiError::internal(format!("MAL parse error: {}", e)))?;
        Ok(Json(SuccessResponse {
            result: "Success".to_string(),
            status: 200,
            data,
        }))
    } else {
        let msg = format!("MAL error: {}", status_code);
        Err(ApiError::internal(msg))
    }
}

/// GET /v2/mal/me
#[utoipa::path(get, path = "/v2/mal/me", tag = "mal", operation_id = "mal_me", responses(
    (status = 200, description = "Success", body = SuccessResponse<MalUser>),
    (status = 400, description = "Bad request", body = ErrorResponseTemplate),
    (status = 500, description = "Internal error", body = ErrorResponseTemplate),
))]
pub async fn me(
    cookies: axum_extra::extract::cookie::CookieJar,
) -> Result<Json<SuccessResponse<MalUser>>, ApiError> {
    let access_token = cookies
        .get("mal_access_token")
        .map(|c| c.value().to_string())
        .ok_or(ApiError::bad_request("Missing access token"))?;

    let client = reqwest::Client::new();
    let resp = client
        .get("https://api.myanimelist.net/v2/users/@me")
        .bearer_auth(&access_token)
        .send()
        .await
        .map_err(|e| ApiError::internal(format!("MAL request failed: {}", e)))?;

    let status_code = resp.status();
    let text = resp.text().await.unwrap_or_default();

    if status_code.is_success() {
        let data: MalUser = serde_json::from_str(&text)
            .map_err(|e| ApiError::internal(format!("MAL parse error: {}", e)))?;
        Ok(Json(SuccessResponse {
            result: "Success".to_string(),
            status: 200,
            data,
        }))
    } else {
        let msg = format!("MAL error: {}", status_code);
        Err(ApiError::internal(msg))
    }
}

/// POST /v2/mal/logout
#[utoipa::path(post, path = "/v2/mal/logout", tag = "mal", operation_id = "mal_logout", responses(
    (status = 200, description = "Success"),
))]
pub async fn logout() -> (axum::http::HeaderMap, Json<SuccessResponse<&'static str>>) {
    let mut headers = axum::http::HeaderMap::new();
    headers.insert(
        axum::http::header::SET_COOKIE,
        delete_cookie("mal_access_token").parse().unwrap(),
    );
    headers.insert(
        axum::http::header::SET_COOKIE,
        delete_cookie("mal_refresh_token").parse().unwrap(),
    );
    let body = Json(SuccessResponse {
        result: "Success".to_string(),
        status: 200,
        data: "Logged out",
    });
    (headers, body)
}
