use crate::core::{ChargeResponse, OrderResponse};
use serde_json::json;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum WebhookError {
    #[error("[Unexpected Webhook Error] {0}")]
    Unexpected(String),
}

mod webhook_rsa2 {
    use openssl::{hash::MessageDigest, pkey::PKey, rsa::Rsa, sign::Signer};

    pub struct SignError(pub String);
    impl From<openssl::error::ErrorStack> for SignError {
        fn from(e: openssl::error::ErrorStack) -> Self {
            SignError(format!("[openssl] {:?}", e))
        }
    }
    impl From<data_encoding::DecodeError> for SignError {
        fn from(e: data_encoding::DecodeError) -> Self {
            SignError(format!("[base64] {:?}", e))
        }
    }

    pub fn sign(sign_source: &str, private_key: &str) -> Result<String, SignError> {
        let keypair = Rsa::private_key_from_pem(private_key.as_bytes())?;
        let keypair = PKey::from_rsa(keypair)?;
        let mut signer = Signer::new(MessageDigest::sha256(), &keypair)?;
        signer.update(sign_source.as_bytes())?;
        let signature_bytes = signer.sign_to_vec()?;
        let signature = data_encoding::BASE64.encode(&signature_bytes);
        Ok(signature)
    }
}

async fn request_to_webhook_endpoint(
    app_webhook_url: &str,
    event_id: &str,
    event_type: &str,
    event_data: &serde_json::Value,
    event_created: i64,
) -> Result<(i32, String), String> {
    let event_payload = json!({
        "id": event_id,
        "object": "event",
        "created": event_created,
        "type": event_type,
        "data": {
            "object": event_data
        },
    });
    let event_payload_text = event_payload.to_string();
    let private_key = std::env::var("WEBHOOK_RSA256_PRIVATE_KEY")
        .expect("WEBHOOK_RSA256_PRIVATE_KEY must be set");
    let signature = webhook_rsa2::sign(&event_payload_text, &private_key)
        .map_err(|e| format!("error signing webhook: {:?}", e.0))?;
    // let app_webhook_url = std::env::var("APP_WEBHOOK_URL").expect("APP_WEBHOOK_URL must be set");
    let res = reqwest::Client::new()
        .post(app_webhook_url)
        .header("X-PingPlusPlus-Signature", signature)
        .header("Content-Type", "application/json")
        .body(event_payload_text)
        // .json(&event_payload)
        .send()
        .await
        .map_err(|e| format!("error sending webhook: {:?}", e))?;
    let status_code = res.status();
    let response_text = res.text().await;
    if status_code != reqwest::StatusCode::OK {
        tracing::error!(
            status = format!("{:?}", status_code),
            text = format!("{:?}", response_text),
            "webhook response from {}",
            app_webhook_url
        );
    } else {
        tracing::info!(
            status = format!("{:?}", status_code),
            text = format!("{:?}", response_text),
            "webhook response from {}",
            app_webhook_url
        );
    }

    Ok((
        status_code.as_u16() as i32,
        response_text.unwrap_or_default(),
    ))
}

pub(super) async fn send_charge_success_webhook(
    prisma_client: &crate::prisma::PrismaClient,
    charge_id: &str,
) -> Result<(), WebhookError> {
    let (charge, order, refunds, app, sub_app) =
        crate::utils::load_charge_from_db(&prisma_client, charge_id)
            .await
            .map_err(|e| WebhookError::Unexpected(e.to_string()))?;

    let webhook_configs = prisma_client
        .app_webhook_config()
        .find_many(vec![crate::prisma::app_webhook_config::app_id::equals(
            app.id.clone(),
        )])
        .exec()
        .await
        .map_err(|e| WebhookError::Unexpected(format!("sql error: {:?}", e)))?;

    for webhook_config in webhook_configs {
        // if let Some(webhook_config) = webhook_config {
        let webhook_config = webhook_config.clone();
        let (event_type, event_data) = match (&order, &sub_app) {
            (Some(order), Some(sub_app)) => {
                let (_, charges, _, _) =
                    crate::utils::load_order_from_db(&prisma_client, &order.id)
                        .await
                        .map_err(|e| WebhookError::Unexpected(e.to_string()))?;
                let order_response: OrderResponse =
                    (order, Some(&charge), &charges, &app, sub_app).into();
                ("order.succeeded", serde_json::to_value(order_response))
            }
            _ => {
                let charge_response: ChargeResponse = (&charge, &refunds, &app).into();
                ("charge.succeeded", serde_json::to_value(charge_response))
            }
        };
        let event_data = match event_data {
            Ok(data) => data,
            Err(e) => {
                let msg = format!("error serializing event data: {:?}", e);
                return Err(WebhookError::Unexpected(msg));
            }
        };

        let record = prisma_client
            .app_webhook_history()
            .create(
                crate::utils::generate_id("evt_"),
                app.id.clone(),
                webhook_config.endpoint.clone(),
                event_type.to_string(),
                event_data,
                0,
                "".to_string(),
                vec![],
            )
            .exec()
            .await
            .map_err(|e| WebhookError::Unexpected(format!("sql error: {:?}", e)))?;

        let webhook_result = request_to_webhook_endpoint(
            &record.endpoint,
            &record.id,
            &record.event,
            &record.payload,
            record.created_at.timestamp(),
        )
        .await;

        match webhook_result {
            Ok((status_code, response_text)) => {
                prisma_client
                    .app_webhook_history()
                    .update(
                        crate::prisma::app_webhook_history::id::equals(record.id.clone()),
                        vec![
                            crate::prisma::app_webhook_history::status_code::set(status_code),
                            crate::prisma::app_webhook_history::response::set(response_text),
                        ],
                    )
                    .exec()
                    .await
                    .map_err(|e| WebhookError::Unexpected(format!("sql error: {:?}", e)))?;
            }
            Err(e) => {
                prisma_client
                    .app_webhook_history()
                    .update(
                        crate::prisma::app_webhook_history::id::equals(record.id.clone()),
                        vec![
                            crate::prisma::app_webhook_history::status_code::set(500),
                            crate::prisma::app_webhook_history::response::set(e),
                        ],
                    )
                    .exec()
                    .await
                    .map_err(|e| WebhookError::Unexpected(format!("sql error: {:?}", e)))?;
            }
        }
    }

    Ok(())
}

pub(super) async fn send_refund_success_webhook(
    _prisma_client: &crate::prisma::PrismaClient,
    _charge_id: &str,
    _refund_id: &str,
) -> Result<(), ()> {
    // 不发送 refund webhook, 现在 ping++ 发送的 order.refunded webhook 没有 refund 信息, 只能靠主动查询
    Ok(())
}
