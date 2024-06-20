use super::super::{
    charge::{
        load_channel_params_from_db, ChannelHandler, CreateChargeRequestPayload, PaymentChannel,
    },
    ChargeError, ChargeStatus,
};
use super::config::{WeixinError, WxPubConfig};
use super::v2api::{self, V2ApiNotifyPayload, V2ApiRequestPayload};
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
        let channel_params =
            load_channel_params_from_db(&prisma_client, &sub_app_id, &PaymentChannel::WxPub)
                .await
                .map_err(|e| WeixinError::InvalidConfig(e))?;
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
        _charge_id: &str,
        order: &crate::prisma::order::Data,
        charge_req_payload: &CreateChargeRequestPayload,
    ) -> Result<serde_json::Value, ChargeError> {
        let config = &self.config;
        let open_id = match charge_req_payload.extra.open_id.as_ref() {
            Some(open_id) => open_id.to_string(),
            None => "".to_string(),
        };
        let mut v2_api_payload = V2ApiRequestPayload::new(
            _charge_id,
            &config.wx_pub_app_id,
            &config.wx_pub_mch_id,
            &open_id,
            &order.client_ip,
            &order.merchant_order_no,
            charge_req_payload.charge_amount,
            order.time_expire,
            &order.subject,
            &order.body,
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

    fn process_notify(&self, payload: &str) -> Result<ChargeStatus, ChargeError> {
        let config = &self.config;
        let notify_payload = V2ApiNotifyPayload::new(payload)?;
        notify_payload.verify_md5_sign(&config.wx_pub_key)?;
        let trade_status = notify_payload.trade_status;
        if trade_status == "SUCCESS" {
            Ok(ChargeStatus::Success)
        } else {
            Ok(ChargeStatus::Fail)
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
