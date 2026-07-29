use axum::Json;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use serde::Serialize;
use serde_json::json;

#[derive(Debug)]
pub enum ApiError {
    Unauthorized {
        message: String,
    },
    Forbidden {
        message: String,
    },
    NotFound(String),
    BadRequest {
        message: String,
        details: Option<String>,
    },
    Conflict {
        message: String,
    },
    Internal {
        message: String,
    },
}

impl From<sqlx::Error> for ApiError {
    fn from(e: sqlx::Error) -> Self {
        ApiError::Internal {
            message: format!("Database error: {}", e),
        }
    }
}

impl ApiError {
    pub fn not_found(msg: impl Into<String>) -> Self {
        ApiError::NotFound(msg.into())
    }
    pub fn bad_request(msg: impl Into<String>) -> Self {
        ApiError::BadRequest {
            message: msg.into(),
            details: None,
        }
    }
    pub fn internal(msg: impl Into<String>) -> Self {
        ApiError::Internal {
            message: msg.into(),
        }
    }
}

#[derive(Debug, Serialize, utoipa::ToSchema)]
#[schema(as = ErrorResponse)]
pub struct ErrorResponseTemplate {
    pub result: String,
    pub status: u16,
    pub data: ErrorData,
}

#[derive(Debug, Serialize, utoipa::ToSchema)]
pub struct ErrorData {
    pub message: String,
    pub details: Option<String>,
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let (status, message, details) = match self {
            ApiError::Unauthorized { message } => (StatusCode::UNAUTHORIZED, message, None),
            ApiError::Forbidden { message } => (StatusCode::FORBIDDEN, message, None),
            ApiError::NotFound(message) => (StatusCode::NOT_FOUND, message, None),
            ApiError::BadRequest { message, details } => {
                (StatusCode::BAD_REQUEST, message, details)
            }
            ApiError::Conflict { message } => (StatusCode::CONFLICT, message, None),
            ApiError::Internal { message } => {
                tracing::error!("Internal error: {}", message);
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "Internal server error".to_string(),
                    None,
                )
            }
        };

        let body = json!({
            "result": "Error",
            "status": status.as_u16(),
            "data": {
                "message": message,
                "details": details,
            }
        });

        (status, Json(body)).into_response()
    }
}
