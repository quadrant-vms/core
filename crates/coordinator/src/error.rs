use axum::{
  Json,
  http::StatusCode,
  response::{IntoResponse, Response},
};
use serde::Serialize;
use std::fmt::{self, Display};

#[derive(Debug)]
pub struct ApiError {
  status: StatusCode,
  message: String,
}

impl ApiError {
  pub fn new(status: StatusCode, message: impl Into<String>) -> Self {
    Self {
      status,
      message: message.into(),
    }
  }

  pub fn bad_request(message: impl Into<String>) -> Self {
    Self::new(StatusCode::BAD_REQUEST, message)
  }

  pub fn internal(message: impl Into<String>) -> Self {
    Self::new(StatusCode::INTERNAL_SERVER_ERROR, message)
  }
}

impl IntoResponse for ApiError {
  fn into_response(self) -> Response {
    let body = Json(ErrorBody {
      error: self.message,
    });
    (self.status, body).into_response()
  }
}

impl Display for ApiError {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    write!(f, "{} ({})", self.message, self.status)
  }
}

impl std::error::Error for ApiError {}

impl From<anyhow::Error> for ApiError {
  fn from(value: anyhow::Error) -> Self {
    Self::internal(value.to_string())
  }
}

#[derive(Serialize)]
struct ErrorBody {
  error: String,
}
