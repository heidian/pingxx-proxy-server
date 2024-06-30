use crate::core::{
    ChannelHandler, ChargeError, ChargeResponse, ChargeStatus, OrderResponse, PaymentChannel,
    RefundError, RefundStatus,
};
use crate::{alipay, weixin};
use serde_json::json;
use std::str::FromStr;

async fn send_charge_webhook(
    app: &crate::prisma::app::Data,
    sub_app: &crate::prisma::sub_app::Data,
    order: &crate::prisma::order::Data,
    charges: &Vec<crate::prisma::charge::Data>,
    charge: &crate::prisma::charge::Data,
) -> Result<(), ()> {
    let order_response: OrderResponse = (
        order.to_owned(),
        charges.to_owned(),
        app.to_owned(),
        sub_app.to_owned(),
    )
        .into();
    // let order_response = OrderResponsePayload::new(&order, &charges, &app, &sub_app);
    let mut event_data = serde_json::to_value(order_response).map_err(|e| {
        tracing::error!("error serializing order response payload: {:?}", e);
    })?;

    let charge_response: ChargeResponse = charge.to_owned().into();
    event_data["charge_essentials"] = serde_json::to_value(charge_response).map_err(|e| {
        tracing::error!("error serializing charge essentials: {:?}", e);
    })?;

    let event_payload = json!({
        "id": crate::utils::generate_id("evt_"),
        "object": "event",
        "created": chrono::Utc::now().timestamp(),
        "type": "order.succeeded",
        "data": {
            "object": event_data
        },
    });

    let app_webhook_url = std::env::var("APP_WEBHOOK_URL").expect("APP_WEBHOOK_URL must be set");
    let res = reqwest::Client::new()
        .post(&app_webhook_url)
        .json(&event_payload)
        .send()
        .await
        .map_err(|e| {
            tracing::error!("error sending webhook: {:?}", e);
        })?;
    tracing::info!("webhook response {} {:?}", res.status(), res.text().await);

    Ok(())
}

async fn process_charge_notify(
    prisma_client: &crate::prisma::PrismaClient,
    charge_id: &str,
    payload: &str,
) -> Result<String, ChargeError> {
    let (charge, order, app, sub_app) =
        crate::utils::load_charge_from_db(&prisma_client, charge_id).await?;

    let channel = PaymentChannel::from_str(&charge.channel).map_err(|e| {
        ChargeError::InternalError(format!("error parsing charge channel: {:?}", e))
    })?;

    let handler: Box<dyn ChannelHandler + Send> = match channel {
        PaymentChannel::AlipayPcDirect => {
            Box::new(alipay::AlipayPcDirect::new(&prisma_client, &sub_app.id).await?)
        }
        PaymentChannel::AlipayWap => {
            Box::new(alipay::AlipayWap::new(&prisma_client, &sub_app.id).await?)
        }
        PaymentChannel::WxPub => Box::new(weixin::WxPub::new(&prisma_client, &sub_app.id).await?),
        PaymentChannel::WxLite => Box::new(weixin::WxLite::new(&prisma_client, &sub_app.id).await?),
    };

    let charge_status = handler.process_charge_notify(payload)?;

    if charge_status == ChargeStatus::Success {
        // update order.paid 并更新 order, 因为后面 send_webhook 需要最新的 order 数据
        prisma_client
            .order()
            .update(
                crate::prisma::order::id::equals(order.id.clone()),
                vec![
                    crate::prisma::order::paid::set(true),
                    crate::prisma::order::time_paid::set(Some(
                        chrono::Utc::now().timestamp() as i32
                    )),
                    crate::prisma::order::amount_paid::set(charge.amount),
                    crate::prisma::order::status::set("paid".to_string()),
                ],
            )
            .exec()
            .await
            .map_err(|e| ChargeError::InternalError(format!("sql error: {:?}", e)))?;

        let (order, charges, _, _) =
            crate::utils::load_order_from_db(&prisma_client, &order.id).await?;
        let _ = send_charge_webhook(&app, &sub_app, &order, &charges, &charge).await;
    }

    match channel {
        PaymentChannel::AlipayPcDirect => {
            Ok("success".to_string())
        }
        PaymentChannel::AlipayWap => {
            Ok("success".to_string())
        }
        PaymentChannel::WxPub => {
            Ok("<xml><return_code><![CDATA[SUCCESS]]></return_code><return_msg><![CDATA[OK]]></return_msg></xml>".to_string())
        }
        PaymentChannel::WxLite => {
            Ok("<xml><return_code><![CDATA[SUCCESS]]></return_code><return_msg><![CDATA[OK]]></return_msg></xml>".to_string())
        }
    }
}

pub async fn create_charge_notify(
    prisma_client: &crate::prisma::PrismaClient,
    charge_id: String,
    notify_payload: String,
) -> Result<String, ChargeError> {
    prisma_client
        .charge_notify_history()
        .create(charge_id.clone(), notify_payload.clone(), vec![])
        .exec()
        .await
        .map_err(|e| ChargeError::InternalError(format!("sql error: {:?}", e)))?;
    let return_body = process_charge_notify(&prisma_client, &charge_id, &notify_payload).await?;
    Ok(return_body)
}

async fn send_refund_webhook(
    _app: &crate::prisma::app::Data,
    _sub_app: &crate::prisma::sub_app::Data,
    _order: &crate::prisma::order::Data,
    _refund: &crate::prisma::refund::Data,
) -> Result<(), ()> {
    // 不发送 refund webhook, 现在 ping++ 发送的 order.refunded webhook 没有 refund 信息, 只能靠主动查询
    Ok(())
}

async fn process_refund_notify(
    prisma_client: &crate::prisma::PrismaClient,
    charge_id: &str,
    refund_id: &str,
    payload: &str,
) -> Result<String, RefundError> {
    let (charge, mut order, app, sub_app) =
        crate::utils::load_charge_from_db(&prisma_client, charge_id).await?;

    let mut refund = prisma_client
        .refund()
        .find_unique(crate::prisma::refund::id::equals(refund_id.to_string()))
        .exec()
        .await
        .map_err(|e| RefundError::Unexpected(format!("sql error: {:?}", e)))?
        .ok_or_else(|| RefundError::BadRequest(format!("refund {} not found", refund_id)))?;

    let channel = PaymentChannel::from_str(&charge.channel)
        .map_err(|e| RefundError::Unexpected(format!("error parsing charge channel: {:?}", e)))?;

    let handler: Box<dyn ChannelHandler + Send> = match channel {
        PaymentChannel::AlipayPcDirect => {
            Box::new(alipay::AlipayPcDirect::new(&prisma_client, &sub_app.id).await?)
        }
        PaymentChannel::AlipayWap => {
            Box::new(alipay::AlipayWap::new(&prisma_client, &sub_app.id).await?)
        }
        PaymentChannel::WxPub => Box::new(weixin::WxPub::new(&prisma_client, &sub_app.id).await?),
        PaymentChannel::WxLite => Box::new(weixin::WxLite::new(&prisma_client, &sub_app.id).await?),
    };

    let refund_status = handler.process_refund_notify(payload)?;

    if refund_status == RefundStatus::Success {
        refund = prisma_client
            .refund()
            .update(
                crate::prisma::refund::id::equals(refund_id.to_string()),
                vec![crate::prisma::refund::status::set(
                    refund_status.to_string(),
                )],
            )
            .exec()
            .await
            .map_err(|e| RefundError::Unexpected(format!("sql error: {:?}", e)))?;
        order = prisma_client
            .order()
            .update(
                crate::prisma::order::id::equals(order.id.clone()),
                vec![
                    crate::prisma::order::refunded::set(true),
                    crate::prisma::order::amount_refunded::increment(refund.amount),
                    crate::prisma::order::status::set("refunded".to_string()),
                ],
            )
            .exec()
            .await
            .map_err(|e| RefundError::Unexpected(format!("sql error: {:?}", e)))?;

        // let (order, charges, _, _) =
        //     crate::utils::load_order_from_db(&prisma_client, &order.id).await?;

        let _ = send_refund_webhook(&app, &sub_app, &order, &refund).await;
    } else if refund_status == RefundStatus::Fail {
        // TODO: 需要把错误信息记录在 refund.failure_msg 上
        let _ = refund;
    }

    match channel {
        PaymentChannel::AlipayPcDirect => {
            Ok("success".to_string())
        }
        PaymentChannel::AlipayWap => {
            Ok("success".to_string())
        }
        PaymentChannel::WxPub => {
            Ok("<xml><return_code><![CDATA[SUCCESS]]></return_code><return_msg><![CDATA[OK]]></return_msg></xml>".to_string())
        }
        PaymentChannel::WxLite => {
            Ok("<xml><return_code><![CDATA[SUCCESS]]></return_code><return_msg><![CDATA[OK]]></return_msg></xml>".to_string())
        }
    }
}

pub async fn create_refund_notify(
    prisma_client: &crate::prisma::PrismaClient,
    charge_id: String,
    refund_id: String,
    notify_payload: String,
) -> Result<String, RefundError> {
    prisma_client
        .charge_notify_history()
        .create(
            charge_id.clone(),
            notify_payload.clone(),
            vec![crate::prisma::charge_notify_history::refund_id::set(Some(
                refund_id.clone(),
            ))],
        )
        .exec()
        .await
        .map_err(|e| RefundError::Unexpected(format!("sql error: {:?}", e)))?;
    let return_body =
        process_refund_notify(&prisma_client, &charge_id, &refund_id, &notify_payload).await?;
    Ok(return_body)
}

pub async fn retry_notify(
    prisma_client: &crate::prisma::PrismaClient,
    id: i32,
) -> Result<String, ()> {
    let history = prisma_client
        .charge_notify_history()
        .find_unique(crate::prisma::charge_notify_history::id::equals(id))
        .exec()
        .await
        .map_err(|e| {
            tracing::error!("sql error: {:?}", e);
        })?
        .ok_or_else(|| {
            tracing::error!("charge notify history {} not found", id);
        })?;
    let charge_id = history.charge_id;
    let refund_id = history.refund_id;
    let charge_notify_payload = history.data;

    if let Some(refund_id) = refund_id {
        let return_body = process_refund_notify(
            &prisma_client,
            &charge_id,
            &refund_id,
            &charge_notify_payload,
        )
        .await
        .map_err(|e| {
            tracing::error!("process_refund_notify error {:?}", e);
        })?;
        return Ok(return_body);
    } else {
        let return_body = process_charge_notify(&prisma_client, &charge_id, &charge_notify_payload)
            .await
            .map_err(|e| {
                tracing::error!("process_charge_notify error {:?}", e);
            })?;
        Ok(return_body)
    }
}

#[allow(unreachable_code)]
#[cfg(test)]
mod tests {
    use super::*;
    use openssl::{hash::MessageDigest, pkey::PKey, rsa::Rsa, sign::Verifier};

    #[tokio::test]
    async fn test_charge_notify_verify_rsa2() {
        return; // skip test
        tracing_subscriber::fmt::init(); // run test with RUST_LOG=info
        let charge_id = "ch_171795983600236120277728";

        let prisma_client = crate::prisma::new_client().await.unwrap();
        let history = prisma_client
            .charge_notify_history()
            .find_first(vec![
                crate::prisma::charge_notify_history::charge_id::equals(charge_id.into()),
            ])
            .exec()
            .await
            .unwrap()
            .unwrap();

        let payload = history.data.clone();
        process_charge_notify(&prisma_client, charge_id, &payload)
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn test_pingxx_signature() {
        return; // skip test
        tracing_subscriber::fmt::init();
        let payload = "";
        let signature = "";
        let api_public_key = "";
        let keypair = Rsa::public_key_from_pem(api_public_key.as_bytes()).unwrap();
        let keypair = PKey::from_rsa(keypair).unwrap();
        let mut verifier = Verifier::new(MessageDigest::sha256(), &keypair).unwrap();
        verifier.update(payload.as_bytes()).unwrap();
        let signature_bytes = data_encoding::BASE64.decode(signature.as_bytes()).unwrap();
        let result = verifier.verify(&signature_bytes).unwrap();
        assert!(result);
        // tracing::info!("verify result: {}", result);
    }

    // 由于没有 ping++ 的私钥，无法以 ping++ 的名义发送 webhook 到业务系统，业务系统需要单独验证从这里发出去的 webhook
}
