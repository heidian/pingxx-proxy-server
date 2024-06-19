use super::super::charge::CreateChargeRequestPayload;
use super::config::{AlipayApiType, AlipayError, AlipayTradeStatus, AlipayWapConfig};
use super::mapi::{MapiNotifyPayload, MapiRequestPayload};
use super::openapi::{OpenApiNotifyPayload, OpenApiRequestPayload};

pub struct AlipayWap {}

impl AlipayWap {
    pub async fn create_credential(
        charge_id: &str,
        config: AlipayWapConfig,
        order: &crate::prisma::order::Data,
        charge_req_payload: &CreateChargeRequestPayload,
    ) -> Result<serde_json::Value, AlipayError> {
        let return_url = match charge_req_payload.extra.success_url.as_ref() {
            Some(url) => url.to_string(),
            None => "".to_string(),
        };
        match config.alipay_version {
            AlipayApiType::MAPI => {
                let mut mapi_request_payload = MapiRequestPayload::new(
                    charge_id,
                    "alipay.wap.create.direct.pay.by.user",
                    &config.alipay_pid,
                    &return_url,
                    &order.merchant_order_no,
                    charge_req_payload.charge_amount,
                    order.time_expire,
                    &order.subject,
                    &order.body,
                )?;
                mapi_request_payload.sign_rsa(&config.alipay_mer_wap_private_key)?;
                serde_json::to_value(mapi_request_payload)
            }
            AlipayApiType::OPENAPI => {
                let mut openapi_request_payload = OpenApiRequestPayload::new(
                    charge_id,
                    "alipay.trade.wap.pay",
                    &config.alipay_app_id,
                    &config.alipay_pid,
                    &return_url,
                    &order.merchant_order_no,
                    charge_req_payload.charge_amount,
                    order.time_expire,
                    &order.subject,
                    &order.body,
                )?;
                openapi_request_payload.sign_rsa2(&config.alipay_mer_wap_private_key_rsa2)?;
                serde_json::to_value(openapi_request_payload)
            }
        }
        .map_err(|e| {
            AlipayError::MalformedPayload(format!("error serializing MapiRequestPayload: {:?}", e))
        })
    }

    pub fn process_notify(config: AlipayWapConfig, payload: &str) -> Result<AlipayTradeStatus, ()> {
        match config.alipay_version {
            AlipayApiType::MAPI => {
                let notify_payload = MapiNotifyPayload::new(payload).map_err(|_| {
                    tracing::error!("invalid notify payload, {}", payload);
                })?;
                let verified = notify_payload
                    .verify_rsa_sign(&config.alipay_wap_public_key)
                    .map_err(|e| {
                        tracing::error!("verify rsa sign: {}", e);
                    })?;
                if !verified {
                    tracing::error!("wrong rsa sign");
                    return Err(());
                }
                // TODO! 需要验证 MapiNotifyPayload 上的 out_trade_no 和 total_fee
                Ok(notify_payload.status)
            }
            AlipayApiType::OPENAPI => {
                let notify_payload = OpenApiNotifyPayload::new(payload).map_err(|_| {
                    tracing::error!("invalid notify payload, {}", payload);
                })?;
                let verified = notify_payload
                    .verify_rsa2_sign(&config.alipay_wap_public_key_rsa2)
                    .map_err(|e| {
                        tracing::error!("verify rsa2 sign: {}", e);
                    })?;
                if !verified {
                    tracing::error!("wrong rsa2 sign");
                    return Err(());
                }
                // TODO! 需要验证 OpenApiNotifyPayload 上的 out_trade_no 和 total_amount
                Ok(notify_payload.status)
            }
        }
    }
}
