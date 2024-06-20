use super::{
    ChargeError,
    ChargeStatus,
    alipay::{self},
    charge::{ChannelHandler, ChargeResponsePayload, PaymentChannel},
    order::OrderResponsePayload,
    weixin::{self},
};
use axum::{
    extract::{Path, Query},
    http::HeaderMap,
    http::StatusCode,
};
use serde_json::json;
use std::str::FromStr;

async fn send_webhook(
    app: &crate::prisma::app::Data,
    sub_app: &crate::prisma::sub_app::Data,
    order: &crate::prisma::order::Data,
    charge: &crate::prisma::charge::Data,
) -> Result<(), ()> {
    let order_response = OrderResponsePayload::new(&order, &app, &sub_app);
    let mut event_data = serde_json::to_value(order_response).map_err(|e| {
        tracing::error!("error serializing order response payload: {:?}", e);
    })?;
    let channel = PaymentChannel::from_str(&charge.channel).map_err(|e| {
        tracing::error!("error parsing charge channel: {:?}", e);
    })?;
    let charge_response = ChargeResponsePayload {
        id: charge.id.clone(),
        object: "charge".to_string(),
        channel,
        amount: charge.amount,
        extra: charge.extra.clone(),
        credential: charge.credential.clone(),
    };
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

async fn process_notify(
    prisma_client: &crate::prisma::PrismaClient,
    charge_id: &str,
    payload: &str,
) -> Result<String, StatusCode> {
    let charge = prisma_client
        .charge()
        .find_unique(crate::prisma::charge::id::equals(charge_id.into()))
        .with(
            crate::prisma::charge::order::fetch()
                .with(crate::prisma::order::sub_app::fetch())
                .with(crate::prisma::order::app::fetch()),
        )
        .exec()
        .await
        .map_err(|e| {
            tracing::error!("sql error: {:?}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?
        .ok_or_else(|| {
            tracing::error!("charge not found: {}", &charge_id);
            StatusCode::NOT_FOUND
        })?;
    let mut order = *charge.order.clone().ok_or_else(|| {
        tracing::error!("order not found for charge {}", &charge_id);
        StatusCode::NOT_FOUND
    })?;
    let app = *order.app.clone().ok_or_else(|| {
        tracing::error!("app not found for charge {}", &charge_id);
        StatusCode::NOT_FOUND
    })?;
    let sub_app = *order.sub_app.clone().ok_or_else(|| {
        tracing::error!("sub_app not found for charge {}", &charge_id);
        StatusCode::NOT_FOUND
    })?;
    let channel = PaymentChannel::from_str(&charge.channel).map_err(|e| {
        tracing::error!("error parsing charge channel: {:?}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    let charge_status = match channel {
        PaymentChannel::AlipayPcDirect => {
            let handler = alipay::AlipayPcDirect::new(&prisma_client, &sub_app.id)
                .await
                .map_err(|e| {
                    tracing::error!("error initializing alipay_pc_direct handler: {:?}", e);
                    StatusCode::INTERNAL_SERVER_ERROR
                })?;
            handler.process_notify(payload).map_err(|e| {
                tracing::error!("error processing alipay notify: {:?}", e);
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
            handler.process_notify(payload).map_err(|e| {
                tracing::error!("error processing alipay notify: {:?}", e);
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
            handler.process_notify(payload).map_err(|e| {
                tracing::error!("error processing weixin notify: {:?}", e);
                StatusCode::INTERNAL_SERVER_ERROR
            })?
        }
    };

    if charge_status == ChargeStatus::Success {
        // update order.paid 并更新 order, 因为后面 send_webhook 需要最新的 order 数据
        order = prisma_client
            .order()
            .update(
                crate::prisma::order::id::equals(order.id),
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
            .map_err(|e| {
                tracing::error!("sql error: {:?}", e);
                StatusCode::INTERNAL_SERVER_ERROR
            })?;

        let _ = send_webhook(&app, &sub_app, &order, &charge).await;
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
    }
}

pub async fn create_charge_notify(
    Query(query): Query<serde_json::Value>,
    Path(charge_id): Path<String>,
    headers: HeaderMap,
    charge_notify_payload: String,
) -> Result<String, StatusCode> {
    let headers_str = format!("{:?}", headers);
    tracing::info!(
        charge_id = charge_id,
        query = query.to_string(),
        payload = charge_notify_payload.as_str(),
        headers = &headers_str,
        "create_charge_notify"
    );
    let prisma_client = crate::prisma::new_client().await.map_err(|e| {
        tracing::error!("error getting prisma client: {:?}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;
    prisma_client
        .charge_notify_history()
        .create(charge_id.clone(), charge_notify_payload.clone(), vec![])
        .exec()
        .await
        .map_err(|e| {
            tracing::error!("sql error: {:?}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    let return_body = process_notify(&prisma_client, &charge_id, &charge_notify_payload).await?;
    Ok(return_body)
}

pub async fn retry_charge_notify(Path(id): Path<i32>) -> Result<String, StatusCode> {
    let prisma_client = crate::prisma::new_client().await.map_err(|e| {
        tracing::error!("error getting prisma client: {:?}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    let history = prisma_client
        .charge_notify_history()
        .find_unique(crate::prisma::charge_notify_history::id::equals(id))
        .exec()
        .await
        .map_err(|e| {
            tracing::error!("sql error: {:?}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?
        .ok_or_else(|| {
            tracing::error!("charge notify history not found: {}", id);
            StatusCode::NOT_FOUND
        })?;
    let charge_id = history.charge_id;
    let charge_notify_payload = history.data;

    let return_body = process_notify(&prisma_client, &charge_id, &charge_notify_payload).await?;
    Ok(return_body)
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
        process_notify(&prisma_client, charge_id, &payload)
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
