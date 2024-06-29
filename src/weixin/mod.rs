mod v2api;
mod wx_pub;
mod wx_lite;

mod config {
    use serde::Deserialize;

    #[derive(Debug, Deserialize)]
    pub struct WxPubConfig {
        // pub finance_version: String,  // 微信新旧资金流，默认值为old，表示旧资金流；new表示新资金流
        pub wx_pub_app_id: String,
        pub wx_pub_mch_id: String,
        pub wx_pub_key: String,
        pub wx_pub_client_cert: String,
        pub wx_pub_client_key: String,
    }

    #[derive(Debug, Deserialize)]
    pub struct WxLiteConfig {
        // pub finance_version: String,  // 微信新旧资金流，默认值为old，表示旧资金流；new表示新资金流
        pub wx_lite_app_id: String,
        pub wx_lite_mch_id: String,
        pub wx_lite_key: String,
        pub wx_lite_client_cert: String,
        pub wx_lite_client_key: String,
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

pub use wx_pub::WxPub;
pub use wx_lite::WxLite;
pub use config::*;
use error::*;
