use httpd::{IntoResponse, Response};
use store::StoreError;

#[derive(Debug)]
pub enum ApiError {
    BadRequest(String),
    NotFound,
    Forbidden(&'static str),
    Internal(String),
}

impl std::fmt::Display for ApiError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::BadRequest(msg) => write!(f, "{msg}"),
            Self::NotFound => write!(f, "not found"),
            Self::Forbidden(msg) => write!(f, "{msg}"),
            Self::Internal(msg) => write!(f, "{msg}"),
        }
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        match self {
            Self::BadRequest(msg) => Response::text(400, msg),
            Self::NotFound => Response::empty(404),
            Self::Forbidden(msg) => Response::text(403, msg),
            Self::Internal(msg) => Response::text(500, msg),
        }
    }
}

impl From<StoreError> for ApiError {
    fn from(e: StoreError) -> Self {
        match e {
            StoreError::NotFound => Self::NotFound,
            e => Self::Internal(e.to_string()),
        }
    }
}

impl From<httpd::BadRequest> for ApiError {
    fn from(e: httpd::BadRequest) -> Self {
        Self::BadRequest(e.0)
    }
}

pub type ApiResult = Result<Response, ApiError>;
