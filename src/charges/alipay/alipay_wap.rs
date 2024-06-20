use super::super::{
    charge::{
        load_channel_params_from_db, ChannelHandler, CreateChargeRequestPayload, PaymentChannel,
    },
    ChargeError, ChargeStatus,
};
use super::config::{AlipayApiType, AlipayError, AlipayTradeStatus, AlipayWapConfig};
use super::mapi::{MapiNotifyPayload, MapiRequestPayload};
use super::openapi::{OpenApiNotifyPayload, OpenApiRequestPayload};
use async_trait::async_trait;

pub struct AlipayWap {
    config: AlipayWapConfig,
}

impl AlipayWap {
    pub async fn new(
        prisma_client: &crate::prisma::PrismaClient,
        sub_app_id: &str,
    ) -> Result<Self, AlipayError> {
        let config = load_channel_params_from_db::<AlipayWapConfig, AlipayError>(
            &prisma_client,
            &sub_app_id,
            &PaymentChannel::AlipayWap,
        )
        .await?;
        Ok(Self { config })
    }
}

#[async_trait]
impl ChannelHandler for AlipayWap {
    async fn create_credential(
        &self,
        charge_id: &str,
        order: &crate::prisma::order::Data,
        charge_req_payload: &CreateChargeRequestPayload,
    ) -> Result<serde_json::Value, ChargeError> {
        let config = &self.config;
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
            ChargeError::MalformedPayload(format!("error serializing MapiRequestPayload: {:?}", e))
        })
    }

    fn process_notify(&self, payload: &str) -> Result<ChargeStatus, ChargeError> {
        let config = &self.config;
        let trade_status = match config.alipay_version {
            AlipayApiType::MAPI => {
                let notify_payload = MapiNotifyPayload::new(payload)?;
                let verified = notify_payload.verify_rsa_sign(&config.alipay_wap_public_key)?;
                if !verified {
                    return Err(ChargeError::MalformedPayload("wrong rsa sign".into()));
                }
                notify_payload.status
            }
            AlipayApiType::OPENAPI => {
                let notify_payload = OpenApiNotifyPayload::new(payload)?;
                let verified =
                    notify_payload.verify_rsa2_sign(&config.alipay_wap_public_key_rsa2)?;
                if !verified {
                    return Err(ChargeError::MalformedPayload("wrong rsa2 sign".into()));
                }
                notify_payload.status
            }
        };
        // TODO! 需要验证 OpenApiNotifyPayload 上的 out_trade_no 和 total_amount
        if trade_status == AlipayTradeStatus::TradeSuccess || trade_status == AlipayTradeStatus::TradeFinished {
            Ok(ChargeStatus::Success)
        } else {
            Ok(ChargeStatus::Fail)
        }
    }
}
