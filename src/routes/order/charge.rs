use crate::core::{
    ChannelChargeExtra, ChannelChargeRequest, ChannelHandler, ChargeError, OrderResponse,
    PaymentChannel,
};
use crate::{alipay, weixin};
use serde::Deserialize;
use serde_json::json;

#[derive(Deserialize, Debug)]
pub struct CreateChargeRequestPayload {
    pub charge_amount: i32,
    pub channel: PaymentChannel,
    pub extra: ChannelChargeExtra,
}

pub async fn create_charge(
    prisma_client: &crate::prisma::PrismaClient,
    order_id: String,
    charge_req_payload: CreateChargeRequestPayload,
) -> Result<serde_json::Value, ChargeError> {
    let charge_id = crate::utils::generate_id("ch_");

    let (order, _charges, app, sub_app) =
        crate::utils::load_order_from_db(&prisma_client, &order_id).await?;

    let handler: Box<dyn ChannelHandler + Send> = match charge_req_payload.channel {
        PaymentChannel::AlipayPcDirect => Box::new(
            alipay::AlipayPcDirect::new(&prisma_client, Some(&app.id), Some(&sub_app.id)).await?,
        ),
        PaymentChannel::AlipayWap => Box::new(
            alipay::AlipayWap::new(&prisma_client, Some(&app.id), Some(&sub_app.id)).await?,
        ),
        PaymentChannel::WxPub => {
            Box::new(weixin::WxPub::new(&prisma_client, Some(&app.id), Some(&sub_app.id)).await?)
        }
        PaymentChannel::WxLite => {
            Box::new(weixin::WxLite::new(&prisma_client, Some(&app.id), Some(&sub_app.id)).await?)
        }
    };

    let credential_object = handler
        .create_credential(&ChannelChargeRequest {
            charge_id: &charge_id,
            charge_amount: charge_req_payload.charge_amount,
            merchant_order_no: &order.merchant_order_no,
            client_ip: &order.client_ip,
            time_expire: order.time_expire,
            subject: &order.subject,
            body: &order.body,
            extra: &charge_req_payload.extra,
        })
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
            crate::prisma::app::id::equals(order.app_id.clone()),
            charge_req_payload.channel.to_string(),
            order.merchant_order_no.clone(),
            false,
            charge_req_payload.charge_amount,
            order.client_ip.clone(),
            order.subject,
            order.body,
            order.currency,
            extra,
            credential,
            order.time_expire,
            vec![crate::prisma::charge::order_id::set(Some(order_id.clone()))],
        )
        .exec()
        .await
        .map_err(|e| ChargeError::InternalError(format!("sql error: {:?}", e)))?;

    // 重新 load 一下 order 数据，因为 order.charges 已经更新
    let (order, charges, _, _) =
        crate::utils::load_order_from_db(&prisma_client, &order_id).await?;
    let order_response: OrderResponse = (&order, Some(&charge), &charges, &app, &sub_app).into();
    let result = serde_json::to_value(order_response).map_err(|e| {
        ChargeError::InternalError(format!("error serializing order response payload: {:?}", e))
    })?;

    Ok(result)
}
