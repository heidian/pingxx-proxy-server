mod error {
    use axum::{
        http::StatusCode,
        response::{IntoResponse, Response},
    };
    use thiserror::Error;

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
}

mod channel {
    use serde::{Deserialize, Serialize};
    use std::{fmt::Debug, str::FromStr};

    #[derive(Deserialize, Serialize, Debug)]
    pub enum PaymentChannel {
        #[serde(rename = "alipay_pc_direct")]
        AlipayPcDirect,
        #[serde(rename = "alipay_wap")]
        AlipayWap,
        #[serde(rename = "wx_pub")]
        WxPub,
    }

    impl FromStr for PaymentChannel {
        type Err = String;
        fn from_str(s: &str) -> Result<Self, Self::Err> {
            let val = serde_json::Value::String(s.to_string());
            let channel = serde_json::from_value::<PaymentChannel>(val)
                .map_err(|e| format!("error parsing PaymentChannel from string: {:?}", e))?;
            Ok(channel)
        }
    }

    impl ToString for PaymentChannel {
        fn to_string(&self) -> String {
            let val = serde_json::to_value(self).unwrap();
            val.as_str().unwrap().to_string()
        }
    }
}

mod request {
    use super::error::ChargeError;
    use async_trait::async_trait;
    use serde::{Deserialize, Serialize};

    #[async_trait]
    pub trait ChannelHandler {
        async fn create_credential(
            &self,
            order: &crate::prisma::order::Data,
            charge_id: &str,
            charge_amount: i32,
            payload: &ChargeExtra,
        ) -> Result<serde_json::Value, ChargeError>;

        fn process_notify(&self, payload: &str) -> Result<ChargeStatus, ChargeError>;
    }

    #[derive(Debug, PartialEq)]
    pub enum ChargeStatus {
        Success,
        Fail,
    }

    #[derive(Deserialize, Serialize, Debug)]
    pub struct ChargeExtra {
        #[serde(skip_serializing_if = "Option::is_none")]
        pub success_url: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub cancel_url: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub open_id: Option<String>,
    }
}

pub use channel::*;
pub use error::*;
pub use request::*;
