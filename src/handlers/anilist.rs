use axum::Json;
use axum::extract::Query;
use serde::Deserialize;
use serde_json::Value;

use crate::error::{ApiError, ErrorResponseTemplate};
use crate::models::anilist::*;
use crate::response::SuccessResponse;

const ANILIST_API_URL: &str = "https://graphql.anilist.co";
const ANILIST_COOKIE_NAME: &str = "ani_access_token";

fn delete_cookie(name: &str) -> String {
    format!("{}=", name)
}

#[derive(Debug, Deserialize, utoipa::IntoParams)]
pub struct AniMangaListQuery {
    #[serde(rename = "userName")]
    pub user_name: String,
}

#[derive(Debug, Deserialize, utoipa::ToSchema, utoipa::IntoParams)]
pub struct AniUpdateMangaListRequest {
    #[serde(rename = "mediaId")]
    pub media_id: i32,
    pub progress: i32,
}

#[derive(Debug, Deserialize, utoipa::IntoParams)]
pub struct TokenQuery {
    pub access_token: Option<String>,
    pub expires_in: Option<i32>,
}

async fn graphql_request(
    access_token: &str,
    query: &str,
    variables: Option<Value>,
) -> Result<(reqwest::StatusCode, Value), ApiError> {
    let client = reqwest::Client::new();
    let mut body = serde_json::json!({ "query": query });
    if let Some(vars) = variables {
        body["variables"] = vars;
    }

    let resp = client
        .post(ANILIST_API_URL)
        .bearer_auth(access_token)
        .json(&body)
        .send()
        .await
        .map_err(|e| ApiError::internal(format!("AniList request failed: {}", e)))?;

    let status = resp.status();
    let text = resp.text().await.unwrap_or_default();
    let data: Value = serde_json::from_str(&text).unwrap_or_default();
    Ok((status, data))
}

/// GET /v2/ani/me
#[utoipa::path(get, path = "/v2/ani/me", tag = "anilist", operation_id = "ani_me", params(TokenQuery), responses(
    (status = 200, description = "Success", body = SuccessResponse<AniViewer>),
    (status = 400, description = "Bad request", body = ErrorResponseTemplate),
    (status = 500, description = "Internal error", body = ErrorResponseTemplate),
))]
pub async fn me(
    cookies: axum_extra::extract::cookie::CookieJar,
    Query(params): Query<TokenQuery>,
) -> Result<Json<SuccessResponse<AniViewer>>, ApiError> {
    let access_token = params
        .access_token
        .or_else(|| {
            cookies
                .get(ANILIST_COOKIE_NAME)
                .map(|c| c.value().to_string())
        })
        .filter(|s| !s.is_empty())
        .ok_or(ApiError::bad_request("Missing access token"))?;

    let expires_in = params.expires_in.unwrap_or(0);

    let query = r#"query { Viewer { id name } }"#;
    let (status, data) = graphql_request(&access_token, query, None).await?;

    if status.is_success() {
        if expires_in > 0 {
            // Cookie will be set via the middleware's Set-Cookie response
            // For now the client must handle it from the response
        }
        let viewer = &data["data"]["Viewer"];
        if viewer.is_null() {
            return Err(ApiError::internal(
                "Invalid response from AniList".to_string(),
            ));
        }
        let viewer: AniViewer = serde_json::from_value(viewer.clone())
            .map_err(|e| ApiError::internal(format!("AniList parse error: {}", e)))?;
        Ok(Json(SuccessResponse {
            result: "Success".to_string(),
            status: 200,
            data: viewer,
        }))
    } else {
        let msg = data["message"]
            .as_str()
            .unwrap_or("Failed to get user info")
            .to_string();
        Err(ApiError::internal(msg))
    }
}

/// POST /v2/ani/logout
#[utoipa::path(post, path = "/v2/ani/logout", tag = "anilist", operation_id = "ani_logout", responses(
    (status = 200, description = "Success"),
))]
pub async fn logout() -> (axum::http::HeaderMap, Json<SuccessResponse<&'static str>>) {
    let mut headers = axum::http::HeaderMap::new();
    headers.insert(
        axum::http::header::SET_COOKIE,
        delete_cookie(ANILIST_COOKIE_NAME).parse().unwrap(),
    );
    let body = Json(SuccessResponse {
        result: "Success".to_string(),
        status: 200,
        data: "Logged out",
    });
    (headers, body)
}

/// GET /v2/ani/mangalist
#[utoipa::path(get, path = "/v2/ani/mangalist", tag = "anilist", operation_id = "ani_get_manga_list", params(AniMangaListQuery), responses(
    (status = 200, description = "Success", body = SuccessResponse<AniMediaListCollection>),
    (status = 400, description = "Bad request", body = ErrorResponseTemplate),
    (status = 500, description = "Internal error", body = ErrorResponseTemplate),
))]
pub async fn get_manga_list(
    cookies: axum_extra::extract::cookie::CookieJar,
    Query(params): Query<AniMangaListQuery>,
) -> Result<Json<SuccessResponse<AniMediaListCollection>>, ApiError> {
    if params.user_name.is_empty() {
        return Err(ApiError::bad_request("userName is required"));
    }

    let access_token = cookies
        .get(ANILIST_COOKIE_NAME)
        .map(|c| c.value().to_string())
        .ok_or(ApiError::bad_request("Missing access token"))?;

    let query = r#"query GetUserMangaList($userName: String, $type: MediaType = MANGA) {
  MediaListCollection(userName: $userName, type: $type) {
    lists {
      name
      entries {
        id
        score
        progress
        status
        media {
          id
          title { english }
        }
      }
    }
  }
}"#;

    let variables = serde_json::json!({
        "userName": params.user_name,
        "type": "MANGA"
    });

    let (status, data) = graphql_request(&access_token, query, Some(variables)).await?;

    if status.is_success() {
        let collection = &data["data"]["MediaListCollection"];
        if collection.is_null() {
            return Err(ApiError::internal(
                "Invalid response from AniList".to_string(),
            ));
        }
        let collection: AniMediaListCollection = serde_json::from_value(collection.clone())
            .map_err(|e| ApiError::internal(format!("AniList parse error: {}", e)))?;
        Ok(Json(SuccessResponse {
            result: "Success".to_string(),
            status: 200,
            data: collection,
        }))
    } else {
        let msg = data["message"]
            .as_str()
            .unwrap_or("Failed to get manga list")
            .to_string();
        Err(ApiError::internal(msg))
    }
}

/// POST /v2/ani/mangalist
#[utoipa::path(post, path = "/v2/ani/mangalist", tag = "anilist", operation_id = "ani_update_manga_list", responses(
    (status = 200, description = "Success", body = SuccessResponse<AniUpdatedEntry>),
    (status = 400, description = "Bad request", body = ErrorResponseTemplate),
    (status = 500, description = "Internal error", body = ErrorResponseTemplate),
))]
pub async fn update_manga_list(
    cookies: axum_extra::extract::cookie::CookieJar,
    Json(body): Json<AniUpdateMangaListRequest>,
) -> Result<Json<SuccessResponse<AniUpdatedEntry>>, ApiError> {
    if body.media_id <= 0 || body.progress < 0 {
        return Err(ApiError::bad_request("Invalid input"));
    }

    let access_token = cookies
        .get(ANILIST_COOKIE_NAME)
        .map(|c| c.value().to_string())
        .ok_or(ApiError::bad_request("Missing access token"))?;

    let query = r#"mutation SaveMangaListEntry($mediaId: Int!, $status: MediaListStatus!, $progress: Int) {
  SaveMediaListEntry(mediaId: $mediaId, status: $status, progress: $progress) {
    id
    status
    progress
  }
}"#;

    let variables = serde_json::json!({
        "mediaId": body.media_id,
        "status": "CURRENT",
        "progress": body.progress
    });

    let (status, data) = graphql_request(&access_token, query, Some(variables)).await?;

    if status.is_success() {
        let entry = &data["data"]["SaveMediaListEntry"];
        if entry.is_null() {
            return Err(ApiError::internal(
                "Invalid response from AniList".to_string(),
            ));
        }
        let entry: AniUpdatedEntry = serde_json::from_value(entry.clone())
            .map_err(|e| ApiError::internal(format!("AniList parse error: {}", e)))?;
        Ok(Json(SuccessResponse {
            result: "Success".to_string(),
            status: 200,
            data: entry,
        }))
    } else {
        let msg = data["message"]
            .as_str()
            .unwrap_or("Failed to update manga list")
            .to_string();
        Err(ApiError::internal(msg))
    }
}
