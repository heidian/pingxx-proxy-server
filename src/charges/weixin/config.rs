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
    MalformedPayload(String), // 请求参数问题
    // #[error("invalid weixin config: {0}")]
    // InvalidConfig(String), // 渠道配置问题
    #[error("unknown: {0}")]
    Unknown(String), // 无法处理的问题
}
