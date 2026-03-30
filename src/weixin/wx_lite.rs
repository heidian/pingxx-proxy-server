use super::{
    v3api::{
        self, V2ApiNotifyPayload, V2ApiRefundNotifyPayload, V2ApiRefundPayload,
        V2ApiRequestPayload, V3JsapiRequestPayload, V3NotifyPayload, V3RefundRequestPayload,
    },
    ApiVersionProbe, WeixinError, WxLiteConfig, WxLiteV2Config, WxLiteV3Config,
};
use crate::core::{
    ChannelChargeRequest, ChannelHandler, ChannelRefundRequest, ChargeError, ChargeStatus,
    PaymentChannel, RefundError, RefundResult, RefundStatus,
};
use async_trait::async_trait;
use serde_json::json;
use std::collections::HashMap;

pub struct WxLite {
    config: WxLiteConfig,
}

impl WxLite {
    pub async fn new(
        prisma_client: &crate::prisma::PrismaClient,
        app_id: Option<&str>,
        sub_app_id: Option<&str>,
    ) -> Result<Self, WeixinError> {
        let channel_params = crate::utils::load_channel_params_from_db(
            &prisma_client,
            app_id,
            sub_app_id,
            &PaymentChannel::WxLite.to_string(),
        )
        .await
        .map_err(|e| WeixinError::InvalidConfig(format!("{:?}", e)))?;

        let probe: ApiVersionProbe = serde_json::from_value(channel_params.params.clone())
            .unwrap_or(ApiVersionProbe { api_version: None });

        let config = if probe.api_version.as_deref() == Some("v3") {
            let v3: WxLiteV3Config =
                serde_json::from_value(channel_params.params).map_err(|e| {
                    WeixinError::InvalidConfig(format!(
                        "error deserializing wx_lite v3 config: {:?}",
                        e
                    ))
                })?;
            WxLiteConfig::V3(v3)
        } else {
            let v2: WxLiteV2Config =
                serde_json::from_value(channel_params.params).map_err(|e| {
                    WeixinError::InvalidConfig(format!(
                        "error deserializing wx_lite v2 config: {:?}",
                        e
                    ))
                })?;
            WxLiteConfig::V2(v2)
        };

        Ok(Self { config })
    }
}

#[async_trait]
impl ChannelHandler for WxLite {
    async fn create_credential(
        &self,
        &ChannelChargeRequest {
            charge_id,
            charge_amount,
            merchant_order_no,
            client_ip,
            time_expire,
            subject,
            body,
            extra,
        }: &ChannelChargeRequest,
    ) -> Result<serde_json::Value, ChargeError> {
        let open_id = extra.open_id.as_deref().ok_or_else(|| {
            ChargeError::MalformedRequest("missing open_id in charge extra".to_string())
        })?;

        match &self.config {
            WxLiteConfig::V2(config) => {
                create_credential_v2(
                    config,
                    charge_id,
                    charge_amount,
                    merchant_order_no,
                    client_ip,
                    time_expire,
                    subject,
                    body,
                    &open_id,
                )
                .await
            }
            WxLiteConfig::V3(config) => {
                create_credential_v3(
                    config,
                    charge_id,
                    charge_amount,
                    merchant_order_no,
                    client_ip,
                    time_expire,
                    subject,
                    body,
                    &open_id,
                )
                .await
            }
        }
    }

    fn process_charge_notify(&self, payload: &str) -> Result<ChargeStatus, ChargeError> {
        match &self.config {
            WxLiteConfig::V2(config) => {
                let notify_payload = V2ApiNotifyPayload::new(payload)?;
                notify_payload.verify_md5_sign(&config.wx_lite_key)?;
                if notify_payload.result_code == "SUCCESS" {
                    Ok(ChargeStatus::Success)
                } else {
                    Ok(ChargeStatus::Fail)
                }
            }
            WxLiteConfig::V3(config) => {
                let notify = V3NotifyPayload::from_json(payload)?;
                let decrypted = notify.decrypt_charge_notify(&config.wx_lite_apiv3_key)?;
                if decrypted.trade_state == "SUCCESS" {
                    Ok(ChargeStatus::Success)
                } else {
                    Ok(ChargeStatus::Fail)
                }
            }
        }
    }

    async fn create_refund(
        &self,
        &ChannelRefundRequest {
            charge_id,
            charge_amount,
            charge_merchant_order_no,
            refund_id,
            refund_amount,
            refund_merchant_order_no,
            description,
            ..
        }: &ChannelRefundRequest,
    ) -> Result<RefundResult, RefundError> {
        match &self.config {
            WxLiteConfig::V2(config) => {
                create_refund_v2(
                    config,
                    charge_id,
                    charge_amount,
                    charge_merchant_order_no,
                    refund_id,
                    refund_amount,
                    refund_merchant_order_no,
                    description,
                )
                .await
            }
            WxLiteConfig::V3(config) => {
                create_refund_v3(
                    config,
                    charge_id,
                    charge_amount,
                    charge_merchant_order_no,
                    refund_id,
                    refund_amount,
                    refund_merchant_order_no,
                    description,
                )
                .await
            }
        }
    }

    fn process_refund_notify(&self, payload: &str) -> Result<RefundStatus, RefundError> {
        match &self.config {
            WxLiteConfig::V2(config) => {
                let notify_payload = V2ApiRefundNotifyPayload::new(payload, &config.wx_lite_key)?;
                // TODO: 需要检查 notify_payload.refund_id 和 notify_payload.amount
                if notify_payload.refund_status == "SUCCESS" {
                    Ok(RefundStatus::Success)
                } else {
                    Ok(RefundStatus::Fail("refund_status != SUCCESS".to_string()))
                }
            }
            WxLiteConfig::V3(config) => {
                let notify = V3NotifyPayload::from_json(payload)?;
                let decrypted = notify.decrypt_refund_notify(&config.wx_lite_apiv3_key)?;
                if decrypted.refund_status == "SUCCESS" {
                    Ok(RefundStatus::Success)
                } else {
                    Ok(RefundStatus::Fail(format!(
                        "refund_status = {}",
                        decrypted.refund_status
                    )))
                }
            }
        }
    }
}

// ============================================================
// V2 实现
// ============================================================

async fn create_credential_v2(
    config: &WxLiteV2Config,
    charge_id: &str,
    charge_amount: i32,
    merchant_order_no: &str,
    client_ip: &str,
    time_expire: i32,
    subject: &str,
    body: &str,
    open_id: &str,
) -> Result<serde_json::Value, ChargeError> {
    let mut v2_api_payload = V2ApiRequestPayload::new(
        charge_id,
        &config.wx_lite_app_id,
        &config.wx_lite_mch_id,
        open_id,
        client_ip,
        merchant_order_no,
        charge_amount,
        time_expire,
        subject,
        body,
    )?;

    v2_api_payload.sign_md5(&config.wx_lite_key)?;
    let res_obj = v2_api_payload.create_prepay_order().await?;

    let mut res_json = json!({
        "appId": res_obj.appid,
        "timeStamp": chrono::Utc::now().timestamp().to_string(),
        "nonceStr": &v2_api_payload.nonce_str,
        "package": format!("prepay_id={}", res_obj.prepay_id.as_ref().unwrap_or(&"".to_string())),
        "signType": "MD5",
    });
    let m: HashMap<String, String> = serde_json::from_value(res_json.clone()).unwrap();
    let signature = v3api::v2api_md5::sign(&m, &config.wx_lite_key);
    res_json["paySign"] = serde_json::Value::String(signature);

    Ok(res_json)
}

// ============================================================
// V3 实现
// ============================================================

async fn create_credential_v3(
    config: &WxLiteV3Config,
    charge_id: &str,
    charge_amount: i32,
    merchant_order_no: &str,
    client_ip: &str,
    time_expire: i32,
    subject: &str,
    body: &str,
    open_id: &str,
) -> Result<serde_json::Value, ChargeError> {
    let payload = V3JsapiRequestPayload::new(
        charge_id,
        &config.wx_lite_app_id,
        &config.wx_lite_mch_id,
        open_id,
        client_ip,
        merchant_order_no,
        charge_amount,
        time_expire,
        subject,
        body,
    )?;

    let res = payload
        .create_prepay_order(
            &config.wx_lite_mch_id,
            &config.wx_lite_serial_no,
            &config.wx_lite_private_key,
        )
        .await?;

    let timestamp = chrono::Utc::now().timestamp().to_string();
    let nonce_str = v3api::v3api_rsa::generate_nonce_str();
    let package = format!("prepay_id={}", res.prepay_id);

    let sign_message = format!(
        "{}\n{}\n{}\n{}\n",
        &config.wx_lite_app_id, &timestamp, &nonce_str, &package
    );
    let pay_sign = v3api::v3api_rsa::sign_sha256_rsa(&sign_message, &config.wx_lite_private_key)?;

    let res_json = json!({
        "appId": &config.wx_lite_app_id,
        "timeStamp": &timestamp,
        "nonceStr": &nonce_str,
        "package": &package,
        "signType": "RSA",
        "paySign": &pay_sign,
    });

    Ok(res_json)
}

async fn create_refund_v2(
    config: &WxLiteV2Config,
    charge_id: &str,
    charge_amount: i32,
    charge_merchant_order_no: &str,
    refund_id: &str,
    refund_amount: i32,
    refund_merchant_order_no: &str,
    description: &str,
) -> Result<RefundResult, RefundError> {
    let mut refund_payload = V2ApiRefundPayload::new(
        refund_id,
        charge_id,
        &config.wx_lite_app_id,
        &config.wx_lite_mch_id,
        charge_merchant_order_no,
        refund_merchant_order_no,
        charge_amount,
        refund_amount,
        description,
    )?;
    refund_payload.sign_md5(&config.wx_lite_key)?;
    let refund_response = refund_payload
        .send_request(&config.wx_lite_client_cert, &config.wx_lite_client_key)
        .await?;
    let mut result = RefundResult {
        amount: refund_amount,
        description: description.to_string(),
        extra: refund_response.clone(),
        ..Default::default()
    };
    let code = refund_response["result_code"].as_str();
    if code == Some("SUCCESS") {
        result.status = RefundStatus::Pending;
    } else {
        result.status = RefundStatus::Fail(format!("code = {:?}", code));
        result.failure_msg = refund_response["err_code_des"]
            .as_str()
            .map(|s| s.to_string());
    }
    Ok(result)
}

async fn create_refund_v3(
    config: &WxLiteV3Config,
    charge_id: &str,
    charge_amount: i32,
    charge_merchant_order_no: &str,
    refund_id: &str,
    refund_amount: i32,
    refund_merchant_order_no: &str,
    description: &str,
) -> Result<RefundResult, RefundError> {
    let refund_payload = V3RefundRequestPayload::new(
        refund_id,
        charge_id,
        charge_merchant_order_no,
        refund_merchant_order_no,
        charge_amount,
        refund_amount,
        description,
    );
    let refund_response = refund_payload
        .send_request(
            &config.wx_lite_mch_id,
            &config.wx_lite_serial_no,
            &config.wx_lite_private_key,
        )
        .await?;

    let mut result = RefundResult {
        amount: refund_amount,
        description: description.to_string(),
        extra: refund_response.extra.clone(),
        ..Default::default()
    };

    match refund_response.status.as_deref() {
        Some("SUCCESS") => {
            result.status = RefundStatus::Success;
        }
        Some("PROCESSING") => {
            result.status = RefundStatus::Pending;
        }
        other => {
            result.status = RefundStatus::Fail(format!("status = {:?}", other));
        }
    }

    Ok(result)
}
