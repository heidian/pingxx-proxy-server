use super::{
    mapi::{MapiNotifyPayload, MapiRefundPayload, MapiRequestPayload},
    openapi::{OpenApiNotifyPayload, OpenApiRefundPayload, OpenApiRequestPayload},
    AlipayApiType, AlipayError, AlipayPcDirectConfig,
};
use crate::core::{
    ChannelChargeRequest, ChannelHandler, ChannelRefundRequest, ChargeError, ChargeStatus,
    PaymentChannel, RefundError, RefundResult, RefundStatus,
};
use async_trait::async_trait;

pub struct AlipayPcDirect {
    config: AlipayPcDirectConfig,
}

impl AlipayPcDirect {
    pub async fn new(
        prisma_client: &crate::prisma::PrismaClient,
        app_id: Option<&str>,
        sub_app_id: Option<&str>,
    ) -> Result<Self, AlipayError> {
        let channel_params = crate::utils::load_channel_params_from_db(
            &prisma_client,
            app_id,
            sub_app_id,
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
        &ChannelChargeRequest {
            charge_id,
            charge_amount,
            merchant_order_no,
            time_expire,
            subject,
            body,
            extra,
            ..
        }: &ChannelChargeRequest,
    ) -> Result<serde_json::Value, ChargeError> {
        let config = &self.config;
        let return_url = match extra.success_url.as_ref() {
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
                    merchant_order_no,
                    charge_amount,
                    time_expire,
                    subject,
                    body,
                )?;
                let private_key = config
                    .alipay_private_key
                    .as_deref()
                    .ok_or(AlipayError::InvalidConfig("missing alipay_private_key".to_string()))?;
                mapi_request_payload.sign_rsa(private_key)?;
                serde_json::to_value(mapi_request_payload)
            }
            AlipayApiType::OPENAPI => {
                let alipay_app_id = config
                    .alipay_app_id
                    .as_deref()
                    .ok_or(AlipayError::InvalidConfig("missing alipay_app_id".to_string()))?;
                let mut openapi_request_payload = OpenApiRequestPayload::new(
                    charge_id,
                    "alipay.trade.page.pay",
                    alipay_app_id,
                    &config.alipay_pid,
                    &return_url,
                    merchant_order_no,
                    charge_amount,
                    time_expire,
                    subject,
                    body,
                )?;
                let private_key = config
                    .alipay_private_key_rsa2
                    .as_deref()
                    .ok_or(AlipayError::InvalidConfig("missing alipay_private_key_rsa2".to_string()))?;
                openapi_request_payload.sign_rsa2(private_key)?;
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
                let public_key = config
                    .alipay_public_key
                    .as_deref()
                    .ok_or(AlipayError::InvalidConfig("missing alipay_public_key".to_string()))?;
                notify_payload.verify_rsa_sign(public_key)?;
                let trade_status = notify_payload.trade_status;
                trade_status == "TRADE_SUCCESS" || trade_status == "TRADE_FINISHED"
            }
            AlipayApiType::OPENAPI => {
                let notify_payload = OpenApiNotifyPayload::new(payload)?;
                let public_key = config
                    .alipay_public_key_rsa2
                    .as_deref()
                    .ok_or(AlipayError::InvalidConfig("missing alipay_public_key_rsa2".to_string()))?;
                notify_payload.verify_rsa2_sign(public_key)?;
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
        &ChannelRefundRequest {
            charge_id,
            charge_merchant_order_no,
            refund_id,
            refund_amount,
            refund_merchant_order_no,
            description,
            // extra,
            ..
        }: &ChannelRefundRequest,
    ) -> Result<RefundResult, RefundError> {
        let config = &self.config;
        let result = match config.alipay_version {
            AlipayApiType::MAPI => {
                let mut refund_payload = MapiRefundPayload::new(
                    refund_id,
                    charge_id,
                    &config.alipay_pid,
                    charge_merchant_order_no,
                    refund_amount,
                    description,
                )?;
                let private_key = config
                    .alipay_private_key
                    .as_deref()
                    .ok_or(AlipayError::InvalidConfig("missing alipay_private_key".to_string()))?;
                refund_payload.sign_rsa(private_key)?;
                // refund_payload.sign_md5(&config.alipay_security_key)?;
                let refund_url = refund_payload.build_refund_url().await?;
                let failure_msg = format!("需要打开地址进行下一步退款操作: {}", refund_url);
                RefundResult {
                    amount: refund_amount,
                    description: description.to_string(),
                    failure_msg: Some(failure_msg),
                    ..Default::default()
                }
            }
            AlipayApiType::OPENAPI => {
                let alipay_app_id = config
                    .alipay_app_id
                    .as_deref()
                    .ok_or(AlipayError::InvalidConfig("missing alipay_app_id".to_string()))?;
                let mut refund_payload = OpenApiRefundPayload::new(
                    alipay_app_id,
                    charge_merchant_order_no,
                    refund_merchant_order_no,
                    refund_amount,
                    description,
                )?;
                let private_key = config
                    .alipay_private_key_rsa2
                    .as_deref()
                    .ok_or(AlipayError::InvalidConfig("missing alipay_private_key_rsa2".to_string()))?;
                refund_payload.sign_rsa2(private_key)?;
                let refund_response = refund_payload.send_request().await?;
                let mut result = RefundResult {
                    amount: refund_amount,
                    description: description.to_string(),
                    extra: refund_response.clone(),
                    ..Default::default()
                };
                let code = refund_response["code"].as_str();
                if code == Some("10000") {
                    if refund_response["fund_change"].as_str() == Some("Y") {
                        result.status = RefundStatus::Success;
                    } else {
                        result.status = RefundStatus::Fail(format!("fund_change != Y"));
                    }
                } else {
                    result.status = RefundStatus::Fail(format!("code = {:?}", code));
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
