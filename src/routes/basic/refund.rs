use crate::core::{
    ChannelHandler, ChannelRefundExtra, ChannelRefundRequest, PaymentChannel, RefundError,
    RefundResponse, RefundStatus,
};
use crate::{alipay, weixin};
use serde::Deserialize;
use std::str::FromStr;

#[derive(Deserialize, Debug)]
pub struct CreateRefundRequestPayload {
    pub amount: i32,
    pub description: String,
    pub funding_source: Option<String>, // 微信退款专用 unsettled_funds | recharge_funds
}

pub async fn create_refund(
    prisma_client: &crate::prisma::PrismaClient,
    charge_id: String,
    refund_req_payload: CreateRefundRequestPayload,
) -> Result<serde_json::Value, RefundError> {
    let refund_id = crate::utils::generate_id("re_");
    let refund_merchant_order_no = refund_id[3..].to_string();

    let (charge, _order, _refunds, app, _sub_app) =
        crate::utils::load_charge_from_db(&prisma_client, &charge_id).await?;

    let channel = PaymentChannel::from_str(&charge.channel).map_err(|e| {
        RefundError::Unexpected(format!(
            "channel {} on refunding charge {} is invalid: {:?}",
            charge.channel, charge.id, e
        ))
    })?;
    let handler: Box<dyn ChannelHandler + Send> = match channel {
        PaymentChannel::AlipayPcDirect => {
            Box::new(alipay::AlipayPcDirect::new(&prisma_client, Some(&app.id), None).await?)
        }
        PaymentChannel::AlipayWap => {
            Box::new(alipay::AlipayWap::new(&prisma_client, Some(&app.id), None).await?)
        }
        PaymentChannel::WxPub => {
            Box::new(weixin::WxPub::new(&prisma_client, Some(&app.id), None).await?)
        }
        PaymentChannel::WxLite => {
            Box::new(weixin::WxLite::new(&prisma_client, Some(&app.id), None).await?)
        }
    };

    let refund_result = handler
        .create_refund(&ChannelRefundRequest {
            charge_id: &charge.id,
            charge_amount: charge.amount,
            charge_merchant_order_no: &charge.merchant_order_no,
            refund_id: &refund_id,
            refund_amount: refund_req_payload.amount,
            refund_merchant_order_no: &refund_merchant_order_no,
            description: &refund_req_payload.description,
            extra: &ChannelRefundExtra {
                funding_source: refund_req_payload.funding_source,
            },
        })
        .await?;

    let refund = prisma_client
        .refund()
        .create(
            refund_id,
            crate::prisma::app::id::equals(charge.app_id.clone()),
            crate::prisma::charge::id::equals(charge_id.clone()),
            refund_merchant_order_no,
            refund_result.status.to_string(),
            refund_result.amount,
            refund_result.description,
            refund_result.extra,
            vec![
                crate::prisma::refund::failure_code::set(refund_result.failure_code),
                crate::prisma::refund::failure_msg::set(refund_result.failure_msg),
            ],
        )
        .exec()
        .await
        .map_err(|e| RefundError::Unexpected(format!("sql error: {:?}", e)))?;

    if refund_result.status == RefundStatus::Success {
        //
    } else if refund_result.status == RefundStatus::Fail {
        //
    } else if refund_result.status == RefundStatus::Pending {
        //
    }

    let refund_response: RefundResponse = (&refund, &charge).into();
    let result = serde_json::to_value(refund_response).map_err(|e| {
        RefundError::Unexpected(format!("error serializing refund response: {:?}", e))
    })?;

    Ok(result)
}

pub async fn retrieve_refund(
    prisma_client: &crate::prisma::PrismaClient,
    charge_id: String,
    refund_id: String,
) -> Result<serde_json::Value, RefundError> {
    let mut refund = prisma_client
        .refund()
        .find_unique(crate::prisma::refund::id::equals(refund_id.to_string()))
        .with(crate::prisma::refund::charge::fetch())
        .exec()
        .await
        .map_err(|e| RefundError::Unexpected(format!("sql error: {:?}", e)))?
        .ok_or_else(|| RefundError::BadRequest(format!("refund {} not found", &refund_id)))?;
    let charge = refund.charge.take().ok_or_else(|| {
        RefundError::Unexpected(format!("failed fetch charge on refund {}", &refund_id))
    })?;
    let charge = *charge;

    if charge.id != charge_id {
        return Err(RefundError::BadRequest(format!(
            "refund {} doesn't belong to charge {}",
            refund_id, charge_id
        )));
    }

    let refund_response: RefundResponse = (&refund, &charge).into();
    let result = serde_json::to_value(refund_response).map_err(|e| {
        RefundError::Unexpected(format!("error serializing refund response: {:?}", e))
    })?;

    Ok(result)
}
