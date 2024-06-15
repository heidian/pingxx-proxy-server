use super::super::charge::CreateChargeRequestPayload;
use super::config::WxPubConfig;
use serde_json::json;
pub struct WxPub {}

impl WxPub {
    pub async fn create_credential(
        config: WxPubConfig,
        order: &crate::prisma::order::Data,
        charge_req_payload: &CreateChargeRequestPayload,
        notify_url: &str,
    ) -> Result<serde_json::Value, ()> {
        Ok(json!({}))
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
