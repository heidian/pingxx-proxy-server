use super::{
    order::OrderResponsePayload, utils::load_order_from_db, ChannelHandler, ChargeError,
    ChargeExtra, PaymentChannel,
};
use crate::{alipay, weixin};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::str::FromStr;

#[derive(Deserialize, Debug)]
pub struct CreateChargeRequestPayload {
    pub charge_amount: i32,
    pub channel: PaymentChannel,
    pub extra: ChargeExtra,
}

#[derive(Serialize, Debug)]
pub struct ChargeResponsePayload {
    pub id: String,
    pub object: String,
    pub channel: PaymentChannel,
    pub amount: i32,
    pub extra: serde_json::Value,
    pub credential: serde_json::Value,
}

pub async fn create_charge(
    prisma_client: &crate::prisma::PrismaClient,
    order_id: String,
    charge_req_payload: CreateChargeRequestPayload,
) -> Result<serde_json::Value, ChargeError> {
    let charge_id = crate::utils::generate_id("ch_");

    let (order, app, sub_app) = load_order_from_db(&prisma_client, &order_id).await?;

    let handler: Box<dyn ChannelHandler + Send> = match charge_req_payload.channel {
        PaymentChannel::AlipayPcDirect => {
            Box::new(alipay::AlipayPcDirect::new(&prisma_client, &sub_app.id).await?)
        }
        PaymentChannel::AlipayWap => {
            Box::new(alipay::AlipayWap::new(&prisma_client, &sub_app.id).await?)
        }
        PaymentChannel::WxPub => Box::new(weixin::WxPub::new(&prisma_client, &sub_app.id).await?),
    };

    let credential_object = handler
        .create_credential(
            &order,
            &charge_id,
            charge_req_payload.charge_amount,
            &charge_req_payload.extra,
        )
        .await?;

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
        ChargeError::MalformedRequest(format!("error serializing charge extra: {:?}", e))
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
        .map_err(|e| ChargeError::InternalError(format!("sql error: {:?}", e)))?;

    let order_response = OrderResponsePayload::new(&order, &app, &sub_app);
    let mut result = serde_json::to_value(order_response).map_err(|e| {
        ChargeError::InternalError(format!("error serializing order response payload: {:?}", e))
    })?;

    let channel = PaymentChannel::from_str(&charge.channel).map_err(|e| {
        ChargeError::MalformedRequest(format!("error parsing charge channel: {:?}", e))
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
        ChargeError::InternalError(format!("error serializing charge essentials: {:?}", e))
    })?;

    Ok(result)
}
