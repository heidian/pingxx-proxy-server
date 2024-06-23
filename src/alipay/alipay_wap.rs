use super::{
    mapi::{MapiNotifyPayload, MapiRequestPayload},
    openapi::{OpenApiNotifyPayload, OpenApiRequestPayload},
    AlipayApiType, AlipayError, AlipayWapConfig,
};
use crate::core::{
    ChannelHandler, ChargeError, ChargeExtra, ChargeStatus, PaymentChannel, RefundError,
    RefundExtra, RefundResult,
};
use async_trait::async_trait;

pub struct AlipayWap {
    config: AlipayWapConfig,
}

impl AlipayWap {
    pub async fn new(
        prisma_client: &crate::prisma::PrismaClient,
        sub_app_id: &str,
    ) -> Result<Self, AlipayError> {
        let channel_params = crate::utils::load_channel_params_from_db(
            &prisma_client,
            &sub_app_id,
            &PaymentChannel::AlipayWap.to_string(),
        )
        .await
        .map_err(|e| AlipayError::InvalidConfig(format!("{:?}", e)))?;
        let config: AlipayWapConfig =
            serde_json::from_value(channel_params.params).map_err(|e| {
                AlipayError::InvalidConfig(
                    format!("error deserializing alipay_wap config: {:?}", e).into(),
                )
            })?;
        Ok(Self { config })
    }
}

#[async_trait]
impl ChannelHandler for AlipayWap {
    async fn create_credential(
        &self,
        order: &crate::prisma::order::Data,
        charge_id: &str,
        charge_amount: i32,
        payload: &ChargeExtra,
    ) -> Result<serde_json::Value, ChargeError> {
        let config = &self.config;
        let return_url = match payload.success_url.as_ref() {
            Some(url) => url.to_string(),
            None => {
                return Err(ChargeError::MalformedRequest(
                    "missing success_url in charge extra".to_string(),
                ))
            }
        };
        let res_json = match config.alipay_version {
            AlipayApiType::MAPI => {
                let mut mapi_request_payload = MapiRequestPayload::new(
                    charge_id,
                    "alipay.wap.create.direct.pay.by.user",
                    &config.alipay_pid,
                    &return_url,
                    &order.merchant_order_no,
                    charge_amount,
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
                    charge_amount,
                    order.time_expire,
                    &order.subject,
                    &order.body,
                )?;
                openapi_request_payload.sign_rsa2(&config.alipay_mer_wap_private_key_rsa2)?;
                serde_json::to_value(openapi_request_payload)
            }
        };
        let res_json = res_json.map_err(|e| {
            AlipayError::Unexpected(format!("error serializing MapiRequestPayload: {:?}", e))
        })?;
        Ok(res_json)
    }

    fn process_notify(&self, payload: &str) -> Result<ChargeStatus, ChargeError> {
        let config = &self.config;
        let success = match config.alipay_version {
            AlipayApiType::MAPI => {
                let notify_payload = MapiNotifyPayload::new(payload)?;
                notify_payload.verify_rsa_sign(&config.alipay_wap_public_key)?;
                let trade_status = notify_payload.trade_status;
                trade_status == "TRADE_SUCCESS" || trade_status == "TRADE_FINISHED"
            }
            AlipayApiType::OPENAPI => {
                let notify_payload = OpenApiNotifyPayload::new(payload)?;
                notify_payload.verify_rsa2_sign(&config.alipay_wap_public_key_rsa2)?;
                let trade_status = notify_payload.trade_status;
                trade_status == "TRADE_SUCCESS" || trade_status == "TRADE_FINISHED"
            }
        };
        // TODO! 需要验证 OpenApiNotifyPayload 上的 out_trade_no 和 total_amount
        if success {
            Ok(ChargeStatus::Success)
        } else {
            Ok(ChargeStatus::Fail)
        }
    }

    async fn create_refund(
        &self,
        _order: &crate::prisma::order::Data,
        _charge: &crate::prisma::charge::Data,
        _refund_amount: i32,
        _payload: &RefundExtra,
    ) -> Result<RefundResult, RefundError> {
        Err(RefundError::Unexpected("not implemented".to_string()))
    }
}
