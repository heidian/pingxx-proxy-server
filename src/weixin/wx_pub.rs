use super::{
    v2api::{
        self, V2ApiNotifyPayload, V2ApiRefundNotifyPayload, V2ApiRefundPayload, V2ApiRequestPayload,
    },
    WeixinError, WxPubConfig,
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
        sub_app_id: &str,
    ) -> Result<Self, WeixinError> {
        let channel_params = crate::utils::load_channel_params_from_db(
            &prisma_client,
            &sub_app_id,
            &PaymentChannel::WxPub.to_string(),
        )
        .await
        .map_err(|e| WeixinError::InvalidConfig(format!("{:?}", e)))?;
        let config: WxPubConfig = serde_json::from_value(channel_params.params).map_err(|e| {
            WeixinError::InvalidConfig(format!("error deserializing wx_pub config: {:?}", e).into())
        })?;
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
        let config = &self.config;
        let open_id = match extra.open_id.as_ref() {
            Some(open_id) => open_id.to_string(),
            None => {
                return Err(ChargeError::MalformedRequest(
                    "missing open_id in charge extra".to_string(),
                ))
            }
        };
        let mut v2_api_payload = V2ApiRequestPayload::new(
            charge_id,
            &config.wx_pub_app_id,
            &config.wx_pub_mch_id,
            &open_id,
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
            // "paySign": "",
        });
        let m: HashMap<String, String> = serde_json::from_value(res_json.to_owned()).unwrap();
        let signature = v2api::v2api_md5::sign(&m, &config.wx_pub_key);
        res_json["paySign"] = serde_json::Value::String(signature);

        Ok(res_json)
    }

    fn process_charge_notify(&self, payload: &str) -> Result<ChargeStatus, ChargeError> {
        let config = &self.config;
        let notify_payload = V2ApiNotifyPayload::new(payload)?;
        notify_payload.verify_md5_sign(&config.wx_pub_key)?;
        let result_code = notify_payload.result_code;
        if result_code == "SUCCESS" {
            Ok(ChargeStatus::Success)
        } else {
            Ok(ChargeStatus::Fail)
        }
    }

    async fn create_refund(
        &self,
        &ChannelRefundRequest {
            charge_id,
            charge_amount,
            refund_id,
            refund_amount,
            merchant_order_no,
            description,
            // extra,
            ..
        }: &ChannelRefundRequest,
    ) -> Result<RefundResult, RefundError> {
        let config = &self.config;
        let mut refund_payload = V2ApiRefundPayload::new(
            refund_id,
            charge_id,
            &config.wx_pub_app_id,
            &config.wx_pub_mch_id,
            merchant_order_no,
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
        if refund_response["result_code"].as_str() == Some("SUCCESS") {
            result.status = RefundStatus::Pending;
        } else {
            result.status = RefundStatus::Fail;
            result.failure_msg = match refund_response["err_code_des"].as_str() {
                Some(msg) => Some(msg.to_string()),
                None => None,
            };
        }
        Ok(result)
    }

    fn process_refund_notify(&self, payload: &str) -> Result<RefundStatus, RefundError> {
        let config = &self.config;
        let notify_payload = V2ApiRefundNotifyPayload::new(payload, &config.wx_pub_key)?;
        let refund_status = notify_payload.refund_status;
        // TODO: 需要检查 notify_payload.refund_id 和 notify_payload.amount
        if refund_status == "SUCCESS" {
            Ok(RefundStatus::Success)
        } else {
            Ok(RefundStatus::Fail)
        }
    }
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
