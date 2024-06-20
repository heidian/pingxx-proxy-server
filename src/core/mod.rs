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

    #[derive(Error, Debug)]
    pub enum DBError {
        #[error("[Error executing SQL] {0}")]
        SQLFailed(String),
        #[error("[DoesNotExist] {0}")]
        DoesNotExist(String),
    }

    impl From<prisma_client_rust::QueryError> for DBError {
        fn from(e: prisma_client_rust::QueryError) -> Self {
            DBError::SQLFailed(format!("{:?}", e))
        }
    }

    impl From<DBError> for ChargeError {
        fn from(e: DBError) -> Self {
            match e {
                DBError::SQLFailed(msg) => ChargeError::InternalError(msg),
                DBError::DoesNotExist(msg) => ChargeError::MalformedRequest(msg),
            }
        }
    }

    impl From<DBError> for OrderError {
        fn from(e: DBError) -> Self {
            match e {
                DBError::SQLFailed(msg) => OrderError::Unexpected(msg),
                DBError::DoesNotExist(msg) => OrderError::BadRequest(msg),
            }
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

pub mod utils {
    use super::{channel::PaymentChannel, error::DBError};

    pub async fn load_order_from_db(
        prisma_client: &crate::prisma::PrismaClient,
        order_id: &str,
    ) -> Result<
        (
            crate::prisma::order::Data,
            crate::prisma::app::Data,
            crate::prisma::sub_app::Data,
        ),
        DBError,
    > {
        let order = prisma_client
            .order()
            .find_unique(crate::prisma::order::id::equals(order_id.to_string()))
            .with(crate::prisma::order::sub_app::fetch())
            .with(crate::prisma::order::app::fetch())
            .with(
                crate::prisma::order::charges::fetch(vec![
                    // crate::prisma::charge::is_valid::equals(true)
                ])
                .order_by(crate::prisma::charge::created_at::order(
                    prisma_client_rust::Direction::Desc,
                )), // .take(1),
            )
            .exec()
            .await?
            .ok_or_else(|| DBError::DoesNotExist(format!("order {}", order_id)))?;

        let (app, sub_app) = {
            let order = order.clone();
            let app = order.app.ok_or_else(|| {
                DBError::SQLFailed(format!("failed fetch app on order {}", order_id))
            })?;
            let sub_app = order.sub_app.ok_or_else(|| {
                DBError::SQLFailed(format!("failed fetch sub_app on order {}", order_id))
            })?;
            (*app, *sub_app)
        };

        Ok((order, app, sub_app))
    }

    pub async fn load_charge_from_db(
        prisma_client: &crate::prisma::PrismaClient,
        charge_id: &str,
    ) -> Result<
        (
            crate::prisma::charge::Data,
            crate::prisma::order::Data,
            crate::prisma::app::Data,
            crate::prisma::sub_app::Data,
        ),
        DBError,
    > {
        let charge = prisma_client
            .charge()
            .find_unique(crate::prisma::charge::id::equals(charge_id.into()))
            .with(
                crate::prisma::charge::order::fetch()
                    .with(crate::prisma::order::sub_app::fetch())
                    .with(crate::prisma::order::app::fetch()),
            )
            .exec()
            .await?
            .ok_or_else(|| DBError::DoesNotExist(format!("charge {}", charge_id)))?;
        let order = charge.order.clone().ok_or_else(|| {
            DBError::SQLFailed(format!("failed fetch order on charge {}", &charge_id))
        })?;
        let app = order.app.clone().ok_or_else(|| {
            DBError::SQLFailed(format!("failed fetch app on charge {}", &charge_id))
        })?;
        let sub_app = order.sub_app.clone().ok_or_else(|| {
            DBError::SQLFailed(format!("failed fetch sub_app on charge {}", &charge_id))
        })?;
        Ok((charge, *order, *app, *sub_app))
    }

    pub async fn load_channel_params_from_db(
        prisma_client: &crate::prisma::PrismaClient,
        sub_app_id: &str,
        channel: &PaymentChannel,
    ) -> Result<crate::prisma::channel_params::Data, DBError> {
        let channel_params = prisma_client
            .channel_params()
            .find_unique(crate::prisma::channel_params::sub_app_id_channel(
                sub_app_id.to_string(),
                channel.to_string(),
            ))
            .exec()
            .await?
            .ok_or_else(|| {
                DBError::DoesNotExist(format!(
                    "channel_params {:?} for sub app {}",
                    channel, sub_app_id
                ))
            })?;
        Ok(channel_params)
    }
}

pub use channel::*;
pub use error::*;
pub use request::*;
