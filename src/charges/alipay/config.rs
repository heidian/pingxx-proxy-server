use serde::Deserialize;
use std::str::FromStr;
use thiserror::Error;

#[derive(Debug)]
pub enum AlipayApiType {
    MAPI,
    OPENAPI,
}

impl<'de> Deserialize<'de> for AlipayApiType {
    fn deserialize<D>(deserializer: D) -> Result<AlipayApiType, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let s = i32::deserialize(deserializer)?;
        match s {
            1 => Ok(AlipayApiType::MAPI),
            2 => Ok(AlipayApiType::OPENAPI),
            _ => Err(serde::de::Error::custom(format!(
                "unknown alipay_api_type: {}",
                s
            ))),
        }
    }
}

/**
 * AlipaySignType 没用到，只做个记录
 * AlipayApiType 决定了要用 RSA 还是 RSA256
 * RSA 对应 MAPI，RSA2 (即 RSA256) 对应 OPENAPI
 */
#[derive(Debug, Deserialize)]
enum AlipaySignType {
    #[serde(rename = "rsa")]
    RSA,
    #[serde(rename = "rsa2")]
    RSA2,
}

#[allow(dead_code)]
#[derive(Debug, Deserialize)]
pub struct AlipayPcDirectConfig {
    pub alipay_pid: String,
    pub alipay_security_key: String,
    pub alipay_account: String,

    pub alipay_version: AlipayApiType,
    pub alipay_app_id: String,

    alipay_sign_type: AlipaySignType,
    pub alipay_private_key: String,
    pub alipay_public_key: String,
    pub alipay_private_key_rsa2: String,
    pub alipay_public_key_rsa2: String,
}

#[allow(dead_code)]
#[derive(Debug, Deserialize)]
pub struct AlipayWapConfig {
    pub alipay_pid: String,
    pub alipay_security_key: String,
    pub alipay_account: String,

    pub alipay_version: AlipayApiType,
    pub alipay_app_id: String,

    alipay_sign_type: AlipaySignType,
    pub alipay_mer_wap_private_key: String,
    pub alipay_wap_public_key: String,
    pub alipay_mer_wap_private_key_rsa2: String,
    pub alipay_wap_public_key_rsa2: String,
}

#[allow(dead_code)]
#[derive(Debug, PartialEq)]
pub enum AlipayTradeStatus {
    TradeSuccess,
    TradeFinished,
}

impl FromStr for AlipayTradeStatus {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "TRADE_SUCCESS" => Ok(AlipayTradeStatus::TradeSuccess),
            "TRADE_FINISHED" => Ok(AlipayTradeStatus::TradeFinished),
            _ => Err(format!("unknown alipay trade status: {}", s)),
        }
    }
}

#[derive(Error, Debug)]
pub enum AlipayError {
    #[error("malformed request payload: {0}")]
    MalformedPayload(String),
    #[error("internal error: {0}")]
    InternalError(String),
}

impl From<openssl::error::ErrorStack> for AlipayError {
    fn from(e: openssl::error::ErrorStack) -> Self {
        AlipayError::InternalError(format!("[openssl] {:?}", e))
    }
}

impl From<data_encoding::DecodeError> for AlipayError {
    fn from(e: data_encoding::DecodeError) -> Self {
        AlipayError::InternalError(format!("[base64] {:?}", e))
    }
}
