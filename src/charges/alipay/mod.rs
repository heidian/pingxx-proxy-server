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
    enum AlipaySignType {
        #[serde(rename = "rsa")]
        RSA,
        #[serde(rename = "rsa2")]
        RSA2,
    }

    #[derive(Debug, Deserialize)]
    pub struct AlipayPcDirectConfig {
        pub alipay_pid: String,
        pub alipay_security_key: String,
        pub alipay_account: String,

        pub alipay_version: AlipayApiType,
        pub alipay_app_id: String,

        // alipay_sign_type: AlipaySignType,
        pub alipay_private_key: String,
        pub alipay_public_key: String,
        pub alipay_private_key_rsa2: String,
        pub alipay_public_key_rsa2: String,
    }

    #[derive(Debug, Deserialize)]
    pub struct AlipayWapConfig {
        pub alipay_pid: String,
        pub alipay_security_key: String,
        pub alipay_account: String,

        pub alipay_version: AlipayApiType,
        pub alipay_app_id: String,

        // alipay_sign_type: AlipaySignType,
        pub alipay_mer_wap_private_key: String,
        pub alipay_wap_public_key: String,
        pub alipay_mer_wap_private_key_rsa2: String,
        pub alipay_wap_public_key_rsa2: String,
    }
}

mod error {
    use crate::charges::ChargeError;
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
}

pub use alipay_pc_direct::AlipayPcDirect;
pub use alipay_wap::AlipayWap;
use config::*;
use error::*;
