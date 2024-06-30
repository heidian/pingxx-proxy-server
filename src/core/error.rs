use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum OrderError {
    #[error("[Bad Order Request Payload] {0}")]
    BadRequest(String),
    #[error("[Unexpected Order Request Error] {0}")]
    Unexpected(String),
}

impl IntoResponse for OrderError {
    fn into_response(self) -> Response {
        tracing::error!("{:?}", self);
        let (status_code, err_msg) = match self {
            OrderError::BadRequest(msg) => (StatusCode::BAD_REQUEST, msg),
            OrderError::Unexpected(msg) => (StatusCode::INTERNAL_SERVER_ERROR, msg),
        };
        (status_code, err_msg).into_response()
    }
}

#[derive(Error, Debug)]
pub enum ChargeError {
    #[error("[Malformed Charge Request] {0}")]
    MalformedRequest(String),
    #[error("[Internal Error] {0}")]
    InternalError(String),
}

impl IntoResponse for ChargeError {
    fn into_response(self) -> Response {
        tracing::error!("{:?}", self);
        let (status_code, err_msg) = match self {
            ChargeError::MalformedRequest(msg) => (StatusCode::BAD_REQUEST, msg),
            ChargeError::InternalError(msg) => (StatusCode::INTERNAL_SERVER_ERROR, msg),
        };
        (status_code, err_msg).into_response()
    }
}

#[derive(Error, Debug)]
pub enum RefundError {
    #[error("[Bad Refund Request Payload] {0}")]
    BadRequest(String),
    #[error("[Unexpected Refund Request Error] {0}")]
    Unexpected(String),
}

impl IntoResponse for RefundError {
    fn into_response(self) -> Response {
        tracing::error!("{:?}", self);
        let (status_code, err_msg) = match self {
            RefundError::BadRequest(msg) => (StatusCode::BAD_REQUEST, msg),
            RefundError::Unexpected(msg) => (StatusCode::INTERNAL_SERVER_ERROR, msg),
        };
        (status_code, err_msg).into_response()
    }
}
