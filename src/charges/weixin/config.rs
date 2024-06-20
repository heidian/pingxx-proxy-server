use serde::Deserialize;
use thiserror::Error;

#[derive(Debug, Deserialize)]
pub struct WxPubConfig {
    pub wx_pub_app_id: String,
    pub wx_pub_mch_id: String,
    pub wx_pub_key: String,
    pub wx_pub_client_cert: String,
    pub wx_pub_client_key: String,
}

#[derive(Error, Debug)]
pub enum WeixinError {
    #[error("[Malformed Weixin Request] {0}")]
    MalformedRequest(String),
    #[error("[Failed Communicating Weixin API] {0}")]
    ApiError(String),
    #[error("[Invalid Weixin Channel Params] {0}")]
    InvalidConfig(String),
    #[error("[Unexpected Weixin Error] {0}")]
    Unexpected(String),
}

impl From<String> for WeixinError {
    fn from(e: String) -> Self {
        WeixinError::Unexpected(e)
    }
}

use crate::charges::ChargeError;
impl From<WeixinError> for ChargeError {
    fn from(e: WeixinError) -> ChargeError {
        tracing::error!("{:?}", e);
        match e {
            WeixinError::MalformedRequest(e) => ChargeError::MalformedRequest(e),
            WeixinError::ApiError(e) => ChargeError::InternalError(e),
            WeixinError::InvalidConfig(e) => ChargeError::InternalError(e),
            WeixinError::Unexpected(e) => ChargeError::InternalError(e),
        }
    }
}
