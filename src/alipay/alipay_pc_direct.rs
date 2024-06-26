use super::{
    mapi::{MapiNotifyPayload, MapiRefundPayload, MapiRequestPayload},
    openapi::{OpenApiNotifyPayload, OpenApiRefundPayload, OpenApiRequestPayload},
    AlipayApiType, AlipayError, AlipayPcDirectConfig,
};
use crate::core::{
    ChannelHandler, ChargeError, ChargeExtra, ChargeStatus, PaymentChannel, RefundError,
    RefundExtra, RefundResult, RefundStatus,
};
use async_trait::async_trait;

pub struct AlipayPcDirect {
    config: AlipayPcDirectConfig,
}

impl AlipayPcDirect {
    pub async fn new(
        prisma_client: &crate::prisma::PrismaClient,
        sub_app_id: &str,
    ) -> Result<Self, AlipayError> {
        let channel_params = crate::utils::load_channel_params_from_db(
            &prisma_client,
            &sub_app_id,
            &PaymentChannel::AlipayPcDirect.to_string(),
        )
        .await
        .map_err(|e| AlipayError::InvalidConfig(format!("{:?}", e)))?;
        let config: AlipayPcDirectConfig =
            serde_json::from_value(channel_params.params).map_err(|e| {
                AlipayError::InvalidConfig(
                    format!("error deserializing alipay_pc_direct config: {:?}", e).into(),
                )
            })?;
        Ok(Self { config })
    }
}

#[async_trait]
impl ChannelHandler for AlipayPcDirect {
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
                    "create_direct_pay_by_user",
                    &config.alipay_pid,
                    return_url.as_str(),
                    &order.merchant_order_no,
                    charge_amount,
                    order.time_expire,
                    &order.subject,
                    &order.body,
                )?;
                mapi_request_payload.sign_rsa(&config.alipay_private_key)?;
                serde_json::to_value(mapi_request_payload)
            }
            AlipayApiType::OPENAPI => {
                let mut openapi_request_payload = OpenApiRequestPayload::new(
                    charge_id,
                    "alipay.trade.page.pay",
                    &config.alipay_app_id,
                    &config.alipay_pid,
                    &return_url,
                    &order.merchant_order_no,
                    charge_amount,
                    order.time_expire,
                    &order.subject,
                    &order.body,
                )?;
                openapi_request_payload.sign_rsa2(&config.alipay_private_key_rsa2)?;
                serde_json::to_value(openapi_request_payload)
            }
        };
        let res_json = res_json.map_err(|e| {
            AlipayError::Unexpected(format!("error serializing MapiRequestPayload: {:?}", e))
        })?;
        Ok(res_json)
    }

    fn process_charge_notify(&self, payload: &str) -> Result<ChargeStatus, ChargeError> {
        let config = &self.config;
        let success = match config.alipay_version {
            AlipayApiType::MAPI => {
                let notify_payload = MapiNotifyPayload::new(payload)?;
                notify_payload.verify_rsa_sign(&config.alipay_public_key)?;
                let trade_status = notify_payload.trade_status;
                trade_status == "TRADE_SUCCESS" || trade_status == "TRADE_FINISHED"
            }
            AlipayApiType::OPENAPI => {
                let notify_payload = OpenApiNotifyPayload::new(payload)?;
                notify_payload.verify_rsa2_sign(&config.alipay_public_key_rsa2)?;
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
        order: &crate::prisma::order::Data,
        charge: &crate::prisma::charge::Data,
        refund_id: &str,
        refund_amount: i32,
        payload: &RefundExtra,
    ) -> Result<RefundResult, RefundError> {
        let config = &self.config;
        let result = match config.alipay_version {
            AlipayApiType::MAPI => {
                let mut refund_payload = MapiRefundPayload::new(
                    refund_id,
                    &charge.id,
                    &config.alipay_pid,
                    &order.merchant_order_no,
                    refund_amount,
                    &payload.description,
                )?;
                refund_payload.sign_rsa(&config.alipay_private_key)?;
                // refund_payload.sign_md5(&config.alipay_security_key)?;
                let refund_url = refund_payload.build_refund_url().await?;
                let failure_msg = format!("需要打开地址进行下一步退款操作: {}", refund_url);
                RefundResult {
                    amount: refund_amount,
                    description: payload.description.clone(),
                    failure_msg: Some(failure_msg),
                    ..Default::default()
                }
            }
            AlipayApiType::OPENAPI => {
                let mut refund_payload = OpenApiRefundPayload::new(
                    &config.alipay_app_id,
                    &order.merchant_order_no,
                    refund_amount,
                    &payload.description,
                )?;
                refund_payload.sign_rsa2(&config.alipay_private_key_rsa2)?;
                let refund_response = refund_payload.send_request().await?;
                let mut result = RefundResult {
                    amount: refund_amount,
                    description: payload.description.clone(),
                    extra: refund_response.clone(),
                    ..Default::default()
                };
                if refund_response["code"].as_str() == Some("10000") {
                    if refund_response["fund_change"].as_str() == Some("Y") {
                        result.status = RefundStatus::Success;
                    } else {
                        result.status = RefundStatus::Fail;
                    }
                } else {
                    result.status = RefundStatus::Fail;
                    result.failure_msg = match refund_response["msg"].as_str() {
                        Some(msg) => Some(msg.to_string()),
                        None => None,
                    };
                }
                result
            }
        };
        Ok(result)
    }

    fn process_refund_notify(&self, _payload: &str) -> Result<RefundStatus, RefundError> {
        Err(RefundError::Unexpected("not implemented".to_string()))
    }
}
