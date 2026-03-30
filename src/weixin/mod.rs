mod v3api;
mod wx_lite;
mod wx_pub;

mod config {
    use serde::Deserialize;

    /// V2 API 配置 (MD5 + XML)
    #[derive(Debug, Deserialize)]
    pub struct WxPubV2Config {
        pub wx_pub_app_id: String,
        pub wx_pub_mch_id: String,
        pub wx_pub_key: String,
        pub wx_pub_client_cert: String,
        pub wx_pub_client_key: String,
    }

    /// V3 API 配置 (RSA-SHA256 + JSON)
    #[derive(Debug, Deserialize)]
    pub struct WxPubV3Config {
        pub wx_pub_app_id: String,
        pub wx_pub_mch_id: String,
        pub wx_pub_apiv3_key: String,   // APIv3 密钥，回调解密用
        pub wx_pub_private_key: String, // 商户 RSA 私钥，签名用
        pub wx_pub_serial_no: String,   // 商户证书序列号
        #[serde(default)]
        pub wx_pub_wechat_public_key: Option<String>, // 微信支付公钥，验签用（可选）
        #[serde(default)]
        pub wx_pub_wechat_public_key_id: Option<String>, // 微信支付公钥 ID（可选）
    }

    #[derive(Debug)]
    pub enum WxPubConfig {
        V2(WxPubV2Config),
        V3(WxPubV3Config),
    }

    /// V2 API 配置 (MD5 + XML)
    #[derive(Debug, Deserialize)]
    pub struct WxLiteV2Config {
        pub wx_lite_app_id: String,
        pub wx_lite_mch_id: String,
        pub wx_lite_key: String,
        pub wx_lite_client_cert: String,
        pub wx_lite_client_key: String,
    }

    /// V3 API 配置 (RSA-SHA256 + JSON)
    #[derive(Debug, Deserialize)]
    pub struct WxLiteV3Config {
        pub wx_lite_app_id: String,
        pub wx_lite_mch_id: String,
        pub wx_lite_apiv3_key: String,
        pub wx_lite_private_key: String,
        pub wx_lite_serial_no: String,
        #[serde(default)]
        pub wx_lite_wechat_public_key: Option<String>,
        #[serde(default)]
        pub wx_lite_wechat_public_key_id: Option<String>,
    }

    #[derive(Debug)]
    pub enum WxLiteConfig {
        V2(WxLiteV2Config),
        V3(WxLiteV3Config),
    }

    /// 用于探测 api_version 字段
    #[derive(Deserialize)]
    pub struct ApiVersionProbe {
        #[serde(default)]
        pub api_version: Option<String>,
    }
}

mod error {
    use crate::core::{ChargeError, RefundError};
    use thiserror::Error;

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

    impl From<WeixinError> for RefundError {
        fn from(e: WeixinError) -> RefundError {
            tracing::error!("{:?}", e);
            match e {
                WeixinError::MalformedRequest(e) => RefundError::BadRequest(e),
                WeixinError::ApiError(e) => RefundError::Unexpected(e),
                WeixinError::InvalidConfig(e) => RefundError::Unexpected(e),
                WeixinError::Unexpected(e) => RefundError::Unexpected(e),
            }
        }
    }
}

pub use config::*;
use error::*;
pub use wx_lite::WxLite;
pub use wx_pub::WxPub;
