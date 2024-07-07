mod alipay_pc_direct;
mod alipay_wap;
mod mapi;
mod openapi;

mod config {
    use serde::Deserialize;

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
    pub enum AlipaySignType {
        #[serde(rename = "rsa")]
        RSA,
        #[serde(rename = "rsa2")]
        RSA2,
    }

    #[derive(Debug, Deserialize)]
    pub struct AlipayPcDirectConfig {
        pub alipay_pid: String,            // 合作者身份, 账号 ID
        pub alipay_security_key: String,   // 安全校验码 (Key)
        pub alipay_account: String,        // 支付宝企业账户（邮箱）
        pub alipay_version: AlipayApiType, // 1:mapi, 2:openapi
        #[serde(default)]
        pub alipay_app_id: String, // 支付宝商户 AppID, alipay_version == 2 时需要
        pub alipay_sign_type: AlipaySignType, // RSA or RSA2, 现在 mapi 固定用 RSA, openapi 固定用 RSA2
        #[serde(default)]
        pub alipay_private_key: String,
        #[serde(default)]
        pub alipay_public_key: String,
        #[serde(default)]
        pub alipay_private_key_rsa2: String,
        #[serde(default)]
        pub alipay_public_key_rsa2: String,
    }

    #[derive(Debug, Deserialize)]
    pub struct AlipayWapConfig {
        pub alipay_pid: String,            // 合作者身份, 账号 ID
        pub alipay_security_key: String,   // 安全校验码 (Key)
        pub alipay_account: String,        // 支付宝企业账户（邮箱）
        pub alipay_version: AlipayApiType, // 1:mapi, 2:openapi
        #[serde(default)]
        pub alipay_app_id: String, // 支付宝商户 AppID, alipay_version == 2 时需要
        pub alipay_sign_type: AlipaySignType, // RSA or RSA2, 现在 mapi 固定用 RSA, openapi 固定用 RSA2
        #[serde(default)]
        pub alipay_mer_wap_private_key: String,
        #[serde(default)]
        pub alipay_wap_public_key: String,
        #[serde(default)]
        pub alipay_mer_wap_private_key_rsa2: String,
        #[serde(default)]
        pub alipay_wap_public_key_rsa2: String,
    }
}

mod error {
    use crate::core::{ChargeError, RefundError};
    use thiserror::Error;

    #[derive(Error, Debug)]
    pub enum AlipayError {
        #[error("[Malformed Alipay Request] {0}")]
        MalformedRequest(String),
        #[error("[Failed Communicating Alipay API] {0}")]
        ApiError(String),
        #[error("[Invalid Alipay Channel Params] {0}")]
        InvalidConfig(String),
        #[error("[Unexpected Alipay Error] {0}")]
        Unexpected(String),
    }

    impl From<openssl::error::ErrorStack> for AlipayError {
        fn from(e: openssl::error::ErrorStack) -> Self {
            AlipayError::Unexpected(format!("[openssl] {:?}", e))
        }
    }

    impl From<data_encoding::DecodeError> for AlipayError {
        fn from(e: data_encoding::DecodeError) -> Self {
            AlipayError::Unexpected(format!("[base64] {:?}", e))
        }
    }

    impl From<AlipayError> for ChargeError {
        fn from(e: AlipayError) -> ChargeError {
            tracing::error!("{:?}", e);
            match e {
                AlipayError::MalformedRequest(e) => ChargeError::MalformedRequest(e),
                AlipayError::ApiError(e) => ChargeError::InternalError(e),
                AlipayError::InvalidConfig(e) => ChargeError::InternalError(e),
                AlipayError::Unexpected(e) => ChargeError::InternalError(e),
            }
        }
    }

    impl From<AlipayError> for RefundError {
        fn from(e: AlipayError) -> RefundError {
            tracing::error!("{:?}", e);
            match e {
                AlipayError::MalformedRequest(e) => RefundError::BadRequest(e),
                AlipayError::ApiError(e) => RefundError::Unexpected(e),
                AlipayError::InvalidConfig(e) => RefundError::Unexpected(e),
                AlipayError::Unexpected(e) => RefundError::Unexpected(e),
            }
        }
    }
}

pub use alipay_pc_direct::AlipayPcDirect;
pub use alipay_wap::AlipayWap;
pub use config::*;
use error::*;
