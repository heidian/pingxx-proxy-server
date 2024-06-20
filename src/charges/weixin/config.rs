use serde::Deserialize;
use std::str::FromStr;
use thiserror::Error;

#[derive(Debug, Deserialize)]
pub struct WxPubConfig {
    pub wx_pub_app_id: String,
    pub wx_pub_mch_id: String,
    pub wx_pub_key: String,
    pub wx_pub_client_cert: String,
    pub wx_pub_client_key: String,
}

#[derive(Debug, PartialEq)]
pub enum WeixinTradeStatus {
    Success,
    Fail,
}

impl FromStr for WeixinTradeStatus {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "SUCCESS" => Ok(WeixinTradeStatus::Success),
            "FAIL" => Ok(WeixinTradeStatus::Fail),
            _ => Err(format!("unknown weixin trade status: {}", s)),
        }
    }
}

#[derive(Error, Debug)]
pub enum WeixinError {
    #[error("malformed request payload: {0}")]
    MalformedPayload(String),
    #[error("internal error: {0}")]
    InternalError(String),
}

impl From<String> for WeixinError {
    fn from(e: String) -> Self {
        WeixinError::InternalError(e)
    }
}

impl From<WeixinError> for crate::charges::ChargeError {
    fn from(e: WeixinError) -> crate::charges::ChargeError {
        match e {
            WeixinError::MalformedPayload(e) => crate::charges::ChargeError::MalformedPayload(e),
            WeixinError::InternalError(e) => crate::charges::ChargeError::InternalError(e),
        }
    }
}
