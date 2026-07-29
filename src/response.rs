use axum::http::StatusCode;
use axum::response::{IntoResponse, Json};
use serde::Serialize;
use utoipa::ToSchema;

#[derive(Serialize, ToSchema)]
pub struct SuccessResponse<T: Serialize + ToSchema> {
    pub result: String,
    pub status: u16,
    pub data: T,
}

impl<T: Serialize + ToSchema> IntoResponse for SuccessResponse<T> {
    fn into_response(self) -> axum::response::Response {
        (StatusCode::OK, Json(self)).into_response()
    }
}

impl<T: Serialize + ToSchema> SuccessResponse<T> {
    pub fn new(data: T) -> Self {
        Self { result: "Success".to_string(), status: 200, data }
    }
}

#[derive(Serialize, ToSchema)]
pub struct ItemsResponse<T: Serialize + ToSchema> {
    pub items: Vec<T>,
}

#[derive(Serialize, ToSchema)]
pub struct PaginatedResponse<T: Serialize + ToSchema> {
    pub items: Vec<T>,
    #[serde(rename = "totalItems")] pub total_items: i64,
    #[serde(rename = "currentPage")] pub current_page: i32,
    #[serde(rename = "pageSize")] pub page_size: i32,
    #[serde(rename = "totalPages")] pub total_pages: i32,
}
