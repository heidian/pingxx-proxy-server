use super::{
    v3api::{
        self, V2ApiNotifyPayload, V2ApiRefundNotifyPayload, V2ApiRefundPayload,
        V2ApiRequestPayload, V3JsapiRequestPayload, V3NotifyPayload, V3RefundRequestPayload,
    },
    ApiVersionProbe, WeixinError, WxPubConfig, WxPubV2Config, WxPubV3Config,
};
use crate::core::{
    ChannelChargeRequest, ChannelHandler, ChannelRefundRequest, ChargeError, ChargeStatus,
    PaymentChannel, RefundError, RefundResult, RefundStatus,
};
use async_trait::async_trait;
use serde_json::json;
use std::collections::HashMap;

pub struct WxPub {
    config: WxPubConfig,
}

impl WxPub {
    pub async fn new(
        prisma_client: &crate::prisma::PrismaClient,
        app_id: Option<&str>,
        sub_app_id: Option<&str>,
    ) -> Result<Self, WeixinError> {
        let channel_params = crate::utils::load_channel_params_from_db(
            &prisma_client,
            app_id,
            sub_app_id,
            &PaymentChannel::WxPub.to_string(),
        )
        .await
        .map_err(|e| WeixinError::InvalidConfig(format!("{:?}", e)))?;

        // 探测 api_version 字段来决定用 V2 还是 V3 配置
        let probe: ApiVersionProbe = serde_json::from_value(channel_params.params.clone())
            .unwrap_or(ApiVersionProbe { api_version: None });

        let config = if probe.api_version.as_deref() == Some("v3") {
            let v3: WxPubV3Config = serde_json::from_value(channel_params.params).map_err(|e| {
                WeixinError::InvalidConfig(format!("error deserializing wx_pub v3 config: {:?}", e))
            })?;
            WxPubConfig::V3(v3)
        } else {
            let v2: WxPubV2Config = serde_json::from_value(channel_params.params).map_err(|e| {
                WeixinError::InvalidConfig(format!("error deserializing wx_pub v2 config: {:?}", e))
            })?;
            WxPubConfig::V2(v2)
        };

        Ok(Self { config })
    }
}

#[async_trait]
impl ChannelHandler for WxPub {
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
            WxPubConfig::V2(config) => {
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
            WxPubConfig::V3(config) => {
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
            WxPubConfig::V2(config) => {
                let notify_payload = V2ApiNotifyPayload::new(payload)?;
                notify_payload.verify_md5_sign(&config.wx_pub_key)?;
                if notify_payload.result_code == "SUCCESS" {
                    Ok(ChargeStatus::Success)
                } else {
                    Ok(ChargeStatus::Fail)
                }
            }
            WxPubConfig::V3(config) => {
                let notify = V3NotifyPayload::from_json(payload)?;
                let decrypted = notify.decrypt_charge_notify(&config.wx_pub_apiv3_key)?;
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
            WxPubConfig::V2(config) => {
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
            WxPubConfig::V3(config) => {
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
            WxPubConfig::V2(config) => {
                let notify_payload = V2ApiRefundNotifyPayload::new(payload, &config.wx_pub_key)?;
                // TODO: 需要检查 notify_payload.refund_id 和 notify_payload.amount
                if notify_payload.refund_status == "SUCCESS" {
                    Ok(RefundStatus::Success)
                } else {
                    Ok(RefundStatus::Fail("refund_status != SUCCESS".to_string()))
                }
            }
            WxPubConfig::V3(config) => {
                let notify = V3NotifyPayload::from_json(payload)?;
                let decrypted = notify.decrypt_refund_notify(&config.wx_pub_apiv3_key)?;
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
    config: &WxPubV2Config,
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
        &config.wx_pub_app_id,
        &config.wx_pub_mch_id,
        open_id,
        client_ip,
        merchant_order_no,
        charge_amount,
        time_expire,
        subject,
        body,
    )?;

    v2_api_payload.sign_md5(&config.wx_pub_key)?;
    let res_obj = v2_api_payload.create_prepay_order().await?;

    /* paySign 不是用前面的 sign, 需要重新生成 */
    let mut res_json = json!({
        "appId": res_obj.appid,
        "timeStamp": chrono::Utc::now().timestamp().to_string(),
        "nonceStr": &v2_api_payload.nonce_str,
        "package": format!("prepay_id={}", res_obj.prepay_id.as_ref().unwrap_or(&"".to_string())),
        "signType": "MD5",
    });
    let m: HashMap<String, String> = serde_json::from_value(res_json.clone()).unwrap();
    let signature = v3api::v2api_md5::sign(&m, &config.wx_pub_key);
    res_json["paySign"] = serde_json::Value::String(signature);

    Ok(res_json)
}

// ============================================================
// V3 实现
// ============================================================

async fn create_credential_v3(
    config: &WxPubV3Config,
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
        &config.wx_pub_app_id,
        &config.wx_pub_mch_id,
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
            &config.wx_pub_mch_id,
            &config.wx_pub_serial_no,
            &config.wx_pub_private_key,
        )
        .await?;

    // V3 JSAPI 调起支付参数
    // https://pay.weixin.qq.com/wiki/doc/apiv3/apis/chapter3_1_4.shtml
    let timestamp = chrono::Utc::now().timestamp().to_string();
    let nonce_str = v3api::v3api_rsa::generate_nonce_str();
    let package = format!("prepay_id={}", res.prepay_id);

    // 签名串: appId\ntimeStamp\nnonceStr\npackage\n
    let sign_message = format!(
        "{}\n{}\n{}\n{}\n",
        &config.wx_pub_app_id, &timestamp, &nonce_str, &package
    );
    let pay_sign = v3api::v3api_rsa::sign_sha256_rsa(&sign_message, &config.wx_pub_private_key)?;

    let res_json = json!({
        "appId": &config.wx_pub_app_id,
        "timeStamp": &timestamp,
        "nonceStr": &nonce_str,
        "package": &package,
        "signType": "RSA",
        "paySign": &pay_sign,
    });

    Ok(res_json)
}

async fn create_refund_v2(
    config: &WxPubV2Config,
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
        &config.wx_pub_app_id,
        &config.wx_pub_mch_id,
        charge_merchant_order_no,
        refund_merchant_order_no,
        charge_amount,
        refund_amount,
        description,
    )?;
    refund_payload.sign_md5(&config.wx_pub_key)?;
    let refund_response = refund_payload
        .send_request(&config.wx_pub_client_cert, &config.wx_pub_client_key)
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
    config: &WxPubV3Config,
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
            &config.wx_pub_mch_id,
            &config.wx_pub_serial_no,
            &config.wx_pub_private_key,
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

#[cfg(test)]
mod tests {
    // 使用 v2 api
    // use super::*;
    // use wechat_pay_rust_sdk::model::JsapiParams;
    // use wechat_pay_rust_sdk::pay::WechatPay;

    // #[tokio::test]
    // async fn test_wx_pub() {
    //     let wechat_pay = WechatPay::new(
    //         "app_id",
    //         "mch_id",
    //         "private_key",
    //         "serial_no",
    //         "v3_key",
    //         "notifi_url",
    //     );
    //     let body = wechat_pay.jsapi_pay(JsapiParams::new(
    //         "测试支付1分",
    //         "1243243",
    //         1.into(),
    //         "open_id".into()
    //         )).await.expect("jsapi_pay error");
    //    println!("body: {:?}", body);
    // }
}
