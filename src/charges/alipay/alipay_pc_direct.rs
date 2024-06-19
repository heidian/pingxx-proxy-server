use super::super::charge::CreateChargeRequestPayload;
use super::config::{AlipayApiType, AlipayError, AlipayPcDirectConfig, AlipayTradeStatus};
use super::mapi::{MapiNotifyPayload, MapiRequestPayload};
use super::openapi::{OpenApiNotifyPayload, OpenApiRequestPayload};
use serde_json::json;

pub struct AlipayPcDirect {}

impl AlipayPcDirect {
    fn create_mapi_credential(
        charge_id: &str,
        config: AlipayPcDirectConfig,
        order: &crate::prisma::order::Data,
        charge_req_payload: &CreateChargeRequestPayload,
    ) -> Result<serde_json::Value, AlipayError> {
        let return_url = match charge_req_payload.extra.success_url.as_ref() {
            Some(url) => url.to_string(),
            None => "".to_string(),
        };
        let mut mapi_request_payload = MapiRequestPayload::new(
            charge_id,
            "create_direct_pay_by_user",
            &config.alipay_pid,
            return_url.as_str(),
            &order.merchant_order_no,
            charge_req_payload.charge_amount,
            order.time_expire,
            &order.subject,
            &order.body,
        )?;
        mapi_request_payload.sign_rsa(&config.alipay_private_key)?;
        Ok(json!(mapi_request_payload))
    }

    fn create_openapi_credential(
        charge_id: &str,
        config: AlipayPcDirectConfig,
        order: &crate::prisma::order::Data,
        charge_req_payload: &CreateChargeRequestPayload,
    ) -> Result<serde_json::Value, AlipayError> {
        let return_url = match charge_req_payload.extra.success_url.as_ref() {
            Some(url) => url.to_string(),
            None => "".to_string(),
        };
        let mut openapi_request_payload = OpenApiRequestPayload::new(
            charge_id,
            "alipay.trade.page.pay",
            &config.alipay_app_id,
            &config.alipay_pid,
            &return_url,
            &order.merchant_order_no,
            charge_req_payload.charge_amount,
            order.time_expire,
            &order.subject,
            &order.body,
        )?;
        openapi_request_payload.sign_rsa2(&config.alipay_private_key_rsa2)?;
        Ok(json!(openapi_request_payload))
    }

    pub async fn create_credential(
        charge_id: &str,
        config: AlipayPcDirectConfig,
        order: &crate::prisma::order::Data,
        charge_req_payload: &CreateChargeRequestPayload,
    ) -> Result<serde_json::Value, AlipayError> {
        match config.alipay_version {
            AlipayApiType::MAPI => {
                Self::create_mapi_credential(charge_id, config, order, charge_req_payload)
            }
            AlipayApiType::OPENAPI => {
                Self::create_openapi_credential(charge_id, config, order, charge_req_payload)
            }
        }
    }

    pub fn process_notify(
        config: AlipayPcDirectConfig,
        payload: &str,
    ) -> Result<AlipayTradeStatus, AlipayError> {
        match config.alipay_version {
            AlipayApiType::MAPI => {
                let notify_payload = MapiNotifyPayload::new(payload)?;
                let verified = notify_payload.verify_rsa_sign(&config.alipay_public_key)?;
                if !verified {
                    return Err(AlipayError::MalformedPayload("wrong rsa sign".into()));
                }
                // TODO! 需要验证 MapiNotifyPayload 上的 out_trade_no 和 total_fee
                Ok(notify_payload.status)
            }
            AlipayApiType::OPENAPI => {
                let notify_payload = OpenApiNotifyPayload::new(payload)?;
                let verified = notify_payload.verify_rsa2_sign(&config.alipay_public_key_rsa2)?;
                if !verified {
                    return Err(AlipayError::MalformedPayload("wrong rsa2 sign".into()));
                }
                // TODO! 需要验证 OpenApiNotifyPayload 上的 out_trade_no 和 total_amount
                Ok(notify_payload.status)
            }
        }
    }
}
