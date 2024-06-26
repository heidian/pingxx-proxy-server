use crate::core::{ChannelHandler, ChargeError, ChargeExtra, PaymentChannel, OrderResponse, ChargeResponse};
use crate::{alipay, weixin};
use serde::Deserialize;
use serde_json::json;

#[derive(Deserialize, Debug)]
pub struct CreateChargeRequestPayload {
    pub charge_amount: i32,
    pub channel: PaymentChannel,
    pub extra: ChargeExtra,
}

pub async fn create_charge(
    prisma_client: &crate::prisma::PrismaClient,
    order_id: String,
    charge_req_payload: CreateChargeRequestPayload,
) -> Result<serde_json::Value, ChargeError> {
    let charge_id = crate::utils::generate_id("ch_");

    let (order, _charges, app, sub_app) = crate::utils::load_order_from_db(&prisma_client, &order_id).await?;

    let handler: Box<dyn ChannelHandler + Send> = match charge_req_payload.channel {
        PaymentChannel::AlipayPcDirect => {
            Box::new(alipay::AlipayPcDirect::new(&prisma_client, &sub_app.id).await?)
        }
        PaymentChannel::AlipayWap => {
            Box::new(alipay::AlipayWap::new(&prisma_client, &sub_app.id).await?)
        }
        PaymentChannel::WxPub => Box::new(weixin::WxPub::new(&prisma_client, &sub_app.id).await?),
        PaymentChannel::WxLite => Box::new(weixin::WxLite::new(&prisma_client, &sub_app.id).await?),
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
            crate::prisma::order::id::equals(order_id.clone()),
            charge_req_payload.channel.to_string(),
            charge_req_payload.charge_amount,
            extra,
            credential,
            vec![],
        )
        .exec()
        .await
        .map_err(|e| ChargeError::InternalError(format!("sql error: {:?}", e)))?;

    // 重新 load 一下 order 数据，因为 order.charges 已经更新
    let (order, charges, _, _) =
        crate::utils::load_order_from_db(&prisma_client, &order_id).await?;
    let order_response: OrderResponse = (
        order.to_owned(),
        charges.to_owned(),
        app.to_owned(),
        sub_app.to_owned(),
    )
        .into();
    let mut result = serde_json::to_value(order_response).map_err(|e| {
        ChargeError::InternalError(format!("error serializing order response payload: {:?}", e))
    })?;

    let charge_response: ChargeResponse = charge.into();
    result["charge_essentials"] = serde_json::to_value(charge_response).map_err(|e| {
        ChargeError::InternalError(format!("error serializing charge: {:?}", e))
    })?;

    Ok(result)
}
