use crate::core::{ChannelHandler, PaymentChannel, RefundError, RefundExtra};
use crate::{alipay, weixin};
use serde::Deserialize;
use serde_json::json;
use std::str::FromStr;

#[derive(Deserialize, Debug)]
pub struct CreateRefundRequestPayload {
    #[serde(rename = "charge")]
    pub charge_id: String,
    #[serde(rename = "charge_amount")]
    pub refund_amount: i32,
    pub description: String,
    pub funding_source: Option<String>, // 微信退款专用 unsettled_funds | recharge_funds
}

pub async fn create_refund(
    prisma_client: &crate::prisma::PrismaClient,
    order_id: String,
    refund_req_payload: CreateRefundRequestPayload,
) -> Result<serde_json::Value, RefundError> {
    tracing::info!("{:?}", refund_req_payload);
    let charge_id = &refund_req_payload.charge_id.clone();
    let (charge, order, app, sub_app) =
        crate::utils::load_charge_from_db(&prisma_client, &charge_id).await?;
    // let (order, app, sub_app) = crate::utils::load_order_from_db(&prisma_client, &order_id).await?;
    if order.id != order_id {
        return Err(RefundError::BadRequest(format!(
            "charge {} doesn't belong to order {}",
            charge_id, order_id
        )));
    }

    let channel = PaymentChannel::from_str(&charge.channel).map_err(|e| {
        RefundError::Unexpected(format!(
            "channel {} on refunding charge {} is invalid: {:?}",
            charge.channel, charge.id, e
        ))
    })?;
    let handler: Box<dyn ChannelHandler + Send> = match channel {
        PaymentChannel::AlipayPcDirect => {
            Box::new(alipay::AlipayPcDirect::new(&prisma_client, &sub_app.id).await?)
        }
        PaymentChannel::AlipayWap => {
            Box::new(alipay::AlipayWap::new(&prisma_client, &sub_app.id).await?)
        }
        PaymentChannel::WxPub => Box::new(weixin::WxPub::new(&prisma_client, &sub_app.id).await?),
    };

    let refund_extra = RefundExtra {
        description: refund_req_payload.description.clone(),
        funding_source: refund_req_payload.funding_source.clone(),
    };

    let _result = handler
        .create_refund(
            &order,
            &charge,
            refund_req_payload.refund_amount,
            &refund_extra,
        )
        .await?;

    let _ = charge;
    let _ = app;
    let _ = sub_app;
    Ok(json!({}))
}
