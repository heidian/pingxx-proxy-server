use super::config::{AlipayApiType, AlipayWapConfig};
use super::mapi::MapiRequestPayload;
use super::openapi::OpenApiRequestPayload;
use crate::charges::charge::CreateChargeRequestPayload;
use crate::orders::Order;
use serde_json::json;

pub struct AlipayWap {}

impl AlipayWap {
    fn load_config() -> Result<AlipayWapConfig, serde_json::error::Error> {
        let pingxx_params = std::env::var("PINGXX_PARAMS").unwrap_or_default();
        let pingxx_params: serde_json::Value = serde_json::from_str(&pingxx_params)?;
        let config: AlipayWapConfig =
            serde_json::from_value(pingxx_params["alipay_wap"].to_owned())?;
        Ok(config)
    }

    fn create_mapi_credential(
        order: &Order,
        charge_req_payload: &CreateChargeRequestPayload,
        notify_url: &str,
        config: AlipayWapConfig,
    ) -> Result<serde_json::Value, ()> {
        let return_url = match charge_req_payload.extra.success_url.as_ref() {
            Some(url) => url.to_string(),
            None => "".to_string(),
        };
        let total_fee = format!("{:.2}", charge_req_payload.charge_amount as f64 / 100.0);
        let it_b_pay = {
            let now = chrono::Utc::now().timestamp() as u32;
            if order.time_expire > now {
                let seconds = order.time_expire - now;
                format!("{}m", if seconds > 60 { seconds / 60 } else { 1 })
            } else {
                tracing::error!("create_credential: expire_in_seconds < now");
                return Err(());
            }
        };
        let mut mapi_request_payload = MapiRequestPayload {
            channel_url: String::from("https://mapi.alipay.com/gateway.do"),
            service: String::from("alipay.wap.create.direct.pay.by.user"),
            _input_charset: String::from("utf-8"),
            return_url,
            notify_url: notify_url.to_string(),
            partner: config.alipay_pid.clone(),
            out_trade_no: order.merchant_order_no.clone(),
            subject: order.subject.clone(),
            body: order.body.clone(),
            total_fee,
            payment_type: String::from("1"),
            seller_id: config.alipay_pid.clone(),
            it_b_pay,
            sign: String::from(""),
            sign_type: String::from("RSA"),
        };
        mapi_request_payload
            .sign_rsa(&config.alipay_mer_wap_private_key)
            .map_err(|e| {
                tracing::error!("create_credential: {}", e);
            })?;
        Ok(json!(mapi_request_payload))
    }

    fn create_openapi_credential(
        order: &Order,
        charge_req_payload: &CreateChargeRequestPayload,
        notify_url: &str,
        config: AlipayWapConfig,
    ) -> Result<serde_json::Value, ()> {
        let return_url = match charge_req_payload.extra.success_url.as_ref() {
            Some(url) => url.to_string(),
            None => "".to_string(),
        };
        let total_amount = format!("{:.2}", charge_req_payload.charge_amount as f64 / 100.0);
        let timeout_express = {
            let now = chrono::Utc::now().timestamp() as u32;
            if order.time_expire > now {
                let seconds = order.time_expire - now;
                format!("{}m", if seconds > 60 { seconds / 60 } else { 1 })
            } else {
                tracing::error!("create_credential: expire_in_seconds < now");
                return Err(());
            }
        };
        let biz_content = json!({
            "body": order.body.clone(),
            "subject": order.subject.clone(),
            "out_trade_no": order.merchant_order_no.clone(),
            "total_amount": total_amount,
            "product_code": "FAST_INSTANT_TRADE_PAY",
            "extend_params": { "sys_service_provider_id": config.alipay_pid.clone() },
            "timeout_express": timeout_express,
            "passback_params": "ch_101240602725900042240014"  // TODO: 这里要换成 charge id
        });
        let mut openapi_request_payload = OpenApiRequestPayload {
            app_id: config.alipay_app_id.clone(),
            method: String::from("alipay.trade.wap.pay"),
            format: String::from("JSON"),
            charset: String::from("utf-8"),
            sign_type: String::from("RSA2"),
            timestamp: chrono::Utc::now().format("%Y-%m-%d %H:%M:%S").to_string(),
            version: String::from("1.0"),
            biz_content: biz_content.to_string(),
            return_url,
            notify_url: notify_url.to_string(),
            sign: String::from(""),
            channel_url: String::from("https://openapi.alipay.com/gateway.do"),
        };
        openapi_request_payload
            .sign_rsa2(&config.alipay_mer_wap_private_key_rsa2)
            .map_err(|e| {
                tracing::error!("create_credential: {}", e);
            })?;
        Ok(json!(openapi_request_payload))
    }

    pub fn create_credential(
        order: &Order,
        charge_req_payload: &CreateChargeRequestPayload,
        notify_url: &str,
    ) -> Result<serde_json::Value, ()> {
        let config = Self::load_config().map_err(|e| {
            tracing::error!("error loading alipay_wap config: {}", e);
        })?;
        match config.alipay_version {
            AlipayApiType::MAPI => {
                Self::create_mapi_credential(order, charge_req_payload, notify_url, config)
            }
            AlipayApiType::OPENAPI => {
                Self::create_openapi_credential(order, charge_req_payload, notify_url, config)
            }
        }
    }
}
