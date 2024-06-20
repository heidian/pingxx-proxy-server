use super::{
    alipay::{self},
    order::{load_order_from_db, OrderResponsePayload},
    weixin::{self},
    ChargeError, ChargeStatus,
};
use async_trait::async_trait;
use axum::{extract::Path, http::StatusCode, response::Json};
use serde::{Deserialize, Serialize};
use serde_json::json;
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

#[derive(Deserialize, Serialize, Debug)]
pub struct ChargeExtra {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub success_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cancel_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub open_id: Option<String>,
}

#[derive(Deserialize, Serialize, Debug)]
pub struct CreateChargeRequestPayload {
    pub charge_amount: i32,
    pub channel: PaymentChannel,
    pub extra: ChargeExtra,
}

#[derive(Deserialize, Serialize, Debug)]
pub struct ChargeResponsePayload {
    pub id: String,
    pub object: String,
    pub channel: PaymentChannel,
    pub amount: i32,
    pub extra: serde_json::Value,
    pub credential: serde_json::Value,
}

pub async fn load_channel_params_from_db(
    prisma_client: &crate::prisma::PrismaClient,
    sub_app_id: &str,
    channel: &PaymentChannel,
) -> Result<crate::prisma::channel_params::Data, String> {
    let channel_params = prisma_client
        .channel_params()
        .find_unique(crate::prisma::channel_params::sub_app_id_channel(
            sub_app_id.to_string(),
            channel.to_string(),
        ))
        .exec()
        .await
        .map_err(|e| format!("sql error: {:?}", e))?
        .ok_or_else(|| format!("channel_params for {:?} not found", channel))?;
    Ok(channel_params)
}

#[async_trait]
pub trait ChannelHandler {
    async fn create_credential(
        &self,
        charge_id: &str,
        order: &crate::prisma::order::Data,
        charge_req_payload: &CreateChargeRequestPayload,
    ) -> Result<serde_json::Value, ChargeError>;

    fn process_notify(&self, payload: &str) -> Result<ChargeStatus, ChargeError>;
}

pub async fn create_charge(
    Path(order_id): Path<String>,
    body: String,
    // Json(charge_req_payload): Json<CreateChargeRequestPayload>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    tracing::info!(order_id, body, "create_charge");
    let charge_req_payload: CreateChargeRequestPayload =
        serde_json::from_str(&body).map_err(|e| {
            tracing::error!("error parsing create_charge request payload: {:?}", e);
            StatusCode::BAD_REQUEST
        })?;
    let charge_id = crate::utils::generate_id("ch_");

    let prisma_client = crate::prisma::new_client().await.map_err(|e| {
        tracing::error!("error getting prisma client: {:?}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    let (order, app, sub_app) = load_order_from_db(&prisma_client, &order_id).await?;

    let credential_object = match charge_req_payload.channel {
        PaymentChannel::AlipayPcDirect => {
            let handler = alipay::AlipayPcDirect::new(&prisma_client, &sub_app.id)
                .await
                .map_err(|e| {
                    tracing::error!("error initializing alipay_pc_direct handler: {:?}", e);
                    StatusCode::INTERNAL_SERVER_ERROR
                })?;
            handler
                .create_credential(&charge_id, &order, &charge_req_payload)
                .await
                .map_err(|e| {
                    tracing::error!("error creating alipay_pc_direct credential: {:?}", e);
                    StatusCode::INTERNAL_SERVER_ERROR
                })?
        }
        PaymentChannel::AlipayWap => {
            let handler = alipay::AlipayWap::new(&prisma_client, &sub_app.id)
                .await
                .map_err(|e| {
                    tracing::error!("error initializing alipay_wap handler: {:?}", e);
                    StatusCode::INTERNAL_SERVER_ERROR
                })?;
            handler
                .create_credential(&charge_id, &order, &charge_req_payload)
                .await
                .map_err(|e| {
                    tracing::error!("error creating alipay_wap credential: {:?}", e);
                    StatusCode::INTERNAL_SERVER_ERROR
                })?
        }
        PaymentChannel::WxPub => {
            let handler = weixin::WxPub::new(&prisma_client, &sub_app.id)
                .await
                .map_err(|e| {
                    tracing::error!("error initializing wx_pub handler: {:?}", e);
                    StatusCode::INTERNAL_SERVER_ERROR
                })?;
            handler
                .create_credential(&charge_id, &order, &charge_req_payload)
                .await
                .map_err(|e| {
                    tracing::error!("error creating wx_pub credential: {:?}", e);
                    StatusCode::INTERNAL_SERVER_ERROR
                })?
        }
    };

    let credential = {
        let mut credential = json!({
            "object": "credential",
            // [channel]: credential_object
        });
        let key = serde_json::to_value(&charge_req_payload.channel)
            .unwrap()
            .as_str()
            .unwrap()
            .to_owned();
        credential[key] = credential_object;
        credential
    };

    let extra = serde_json::to_value(charge_req_payload.extra).map_err(|e| {
        tracing::error!("error serializing charge extra: {:?}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    let charge = prisma_client
        .charge()
        .create(
            charge_id.clone(),
            crate::prisma::order::id::equals(order_id),
            charge_req_payload.channel.to_string(),
            charge_req_payload.charge_amount,
            extra,
            credential,
            vec![],
        )
        .exec()
        .await
        .map_err(|e| {
            tracing::error!("error creating charge: {:?}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    let order_response = OrderResponsePayload::new(&order, &app, &sub_app);
    let mut result = serde_json::to_value(order_response).map_err(|e| {
        tracing::error!("error serializing order response payload: {:?}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    let channel = PaymentChannel::from_str(&charge.channel).map_err(|e| {
        tracing::error!("error parsing charge channel: {:?}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;
    let charge_response = ChargeResponsePayload {
        id: charge.id,
        object: "charge".to_string(),
        channel,
        amount: charge.amount,
        extra: charge.extra,
        credential: charge.credential,
    };
    result["charge_essentials"] = serde_json::to_value(charge_response).map_err(|e| {
        tracing::error!("error serializing charge essentials: {:?}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    Ok(Json(result))
}
