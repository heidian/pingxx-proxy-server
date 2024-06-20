mod alipay;
mod charge;
mod notify;
mod order;
mod weixin;

mod routes;
pub use routes::get_routes;

use thiserror::Error;

#[derive(Debug, PartialEq)]
pub enum ChargeStatus {
    Success,
    Fail,
}

#[derive(Error, Debug)]
pub enum ChargeError {
    #[error("malformed request payload: {0}")]
    MalformedPayload(String),
    #[error("internal error: {0}")]
    InternalError(String),
}

impl From<openssl::error::ErrorStack> for ChargeError {
    fn from(e: openssl::error::ErrorStack) -> Self {
        ChargeError::InternalError(format!("[openssl] {:?}", e))
    }
}

impl From<data_encoding::DecodeError> for ChargeError {
    fn from(e: data_encoding::DecodeError) -> Self {
        ChargeError::InternalError(format!("[base64] {:?}", e))
    }
}

impl From<String> for ChargeError {
    fn from(e: String) -> Self {
        ChargeError::InternalError(e)
    }
}
