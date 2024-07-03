use crate::core::{
    ChannelHandler, ChannelRefundExtra, ChannelRefundRequest, PaymentChannel, RefundError,
    RefundStatus,
};
use crate::{alipay, weixin};
use serde::Deserialize;
use serde_json::json;
use std::str::FromStr;

#[derive(Deserialize, Debug)]
pub struct CreateRefundRequestPayload {
    pub charge_id: String,
    pub refund_amount: i32,
    pub description: String,
    pub funding_source: Option<String>, // 微信退款专用 unsettled_funds | recharge_funds
}

pub async fn create_refund(
    prisma_client: &crate::prisma::PrismaClient,
    order_id: String,
    refund_req_payload: CreateRefundRequestPayload,
) -> Result<serde_json::Value, RefundError> {
    let refund_id = crate::utils::generate_id("re_");
    let merchant_order_no = refund_id[3..].to_string();

    let charge_id = &refund_req_payload.charge_id.clone();
    let (charge, order, app, sub_app) =
        crate::utils::load_charge_from_db(&prisma_client, &charge_id).await?;

    let order = match order {
        Some(order) => order,
        None => return Err(RefundError::BadRequest(format!("order not found on charge {}", charge_id))),
    };

    let sub_app = match sub_app {
        Some(sub_app) => sub_app,
        None => return Err(RefundError::BadRequest(format!("sub_app not found on order {}", charge_id))),
    };

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

    let refund_result = handler
        .create_refund(&ChannelRefundRequest {
            charge_id: &charge.id,
            charge_amount: charge.amount,
            refund_id: &refund_id,
            refund_amount: refund_req_payload.refund_amount,
            merchant_order_no: &merchant_order_no,
            description: &refund_req_payload.description,
            extra: &ChannelRefundExtra {
                funding_source: refund_req_payload.funding_source,
            },
        })
        .await?;

    let refund = prisma_client
        .refund()
        .create(
            refund_id.clone(),
            crate::prisma::app::id::equals(order.app_id.clone()),
            crate::prisma::charge::id::equals(charge_id.clone()),
            merchant_order_no.clone(),
            refund_result.status.to_string(),
            refund_result.amount,
            refund_result.description,
            refund_result.extra,
            vec![
                crate::prisma::refund::order_id::set(Some(order_id.clone())),
                crate::prisma::refund::failure_code::set(refund_result.failure_code),
                crate::prisma::refund::failure_msg::set(refund_result.failure_msg),
            ],
        )
        .exec()
        .await
        .map_err(|e| RefundError::Unexpected(format!("sql error: {:?}", e)))?;

    if refund_result.status == RefundStatus::Success {
        prisma_client
            .order()
            .update(
                crate::prisma::order::id::equals(order_id.clone()),
                vec![
                    crate::prisma::order::refunded::set(true),
                    crate::prisma::order::amount_refunded::increment(refund_result.amount),
                    crate::prisma::order::status::set("refunded".to_string()),
                ],
            )
            .exec()
            .await
            .map_err(|e| RefundError::Unexpected(format!("sql error: {:?}", e)))?;
    } else if refund_result.status == RefundStatus::Fail {
        //
    } else if refund_result.status == RefundStatus::Pending {
        //
    }

    let res_json = json!({
        "object": "list",
        "url": format!("/v1/orders/{}/order_refunds", &order_id),
        "has_more": false,
        "data": [{
            "id": &refund.id,
            "object": "refund",
            "amount": &refund.amount,
            "succeed": refund_result.status == RefundStatus::Success,
            "status": &refund.status,
            "description": &refund.description,
            "failure_code": &refund.failure_code,
            "failure_msg": &refund.failure_msg,
            "metadata": {},
            "charge": &charge.id,
            "charge_order_no": &order.merchant_order_no,
            "extra": &refund.extra
        }]
    });
    Ok(res_json)
}

pub async fn retrieve_refund(
    prisma_client: &crate::prisma::PrismaClient,
    order_id: String,
    refund_id: String,
) -> Result<serde_json::Value, RefundError> {
    let refund = prisma_client
        .refund()
        .find_unique(crate::prisma::refund::id::equals(refund_id.to_string()))
        .with(crate::prisma::refund::charge::fetch())
        .with(crate::prisma::refund::order::fetch())
        .exec()
        .await
        .map_err(|e| RefundError::Unexpected(format!("sql error: {:?}", e)))?
        .ok_or_else(|| RefundError::BadRequest(format!("refund {} not found", &refund_id)))?;
    let (charge, order) = {
        let refund = refund.clone();
        let charge = refund.charge.ok_or_else(|| {
            RefundError::Unexpected(format!("failed fetch charge on refund {}", &refund_id))
        })?;
        let order = refund
            .order
            .ok_or_else(|| {
                RefundError::Unexpected(format!("failed fetch order on refund {}", &refund_id))
            })?
            .ok_or_else(|| {
                RefundError::Unexpected(format!("order not found on refund {}", &refund_id))
            })?;
        (*charge, *order)
    };

    if order.id != order_id {
        return Err(RefundError::BadRequest(format!(
            "refund {} doesn't belong to order {}",
            refund_id, order_id
        )));
    }

    Ok(json!({
        "id": refund.id,
        "object": "refund",
        "order_no": refund.id,
        "amount": refund.amount,
        "created": refund.created_at.timestamp(),
        "succeed": refund.status == "succeeded",
        "status": refund.status,
        "description": refund.description,
        "failure_code": refund.failure_code,
        "failure_msg": refund.failure_msg,
        "charge": charge.id,
        "charge_order_no": order.merchant_order_no,
        "funding_source": null,
        "extra": refund.extra,
    }))
}
