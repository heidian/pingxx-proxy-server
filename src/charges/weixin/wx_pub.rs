use super::super::charge::CreateChargeRequestPayload;
use super::config::{WeixinError, WeixinTradeStatus, WxPubConfig};
use super::v2api::{self, V2ApiNotifyPayload, V2ApiRequestPayload};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::HashMap;

#[derive(Deserialize, Serialize, Debug)]
struct WxJSAPIResponse {
    return_code: String,
    return_msg: String,

    appid: Option<String>,
    mch_id: Option<String>,
    nonce_str: Option<String>,
    sign: Option<String>,
    result_code: Option<String>,
    err_code: Option<String>,
    err_code_des: Option<String>,

    trade_type: Option<String>,
    prepay_id: Option<String>,
}

pub struct WxPub {}

impl WxPub {
    /**
     * https://pay.weixin.qq.com/wiki/doc/api/jsapi.php?chapter=7_7&index=6
     * https://pay.weixin.qq.com/wiki/doc/api/jsapi.php?chapter=9_1
     */
    pub async fn create_credential(
        _charge_id: &str,
        config: WxPubConfig,
        order: &crate::prisma::order::Data,
        charge_req_payload: &CreateChargeRequestPayload,
        notify_url: &str,
    ) -> Result<serde_json::Value, WeixinError> {
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
            notify_url,
            &order.merchant_order_no,
            charge_req_payload.charge_amount,
            order.time_expire,
            &order.subject,
            &order.body,
        )?;

        v2_api_payload.sign_md5(&config.wx_pub_key)?;

        let xml_payload = quick_xml::se::to_string_with_root("xml", &v2_api_payload)
            .map_err(|e| WeixinError::MalformedPayload(format!("malformed xml payload: {}", e)))?;

        let res = reqwest::Client::new()
            .post("https://api.mch.weixin.qq.com/pay/unifiedorder")
            .body(xml_payload)
            .send()
            .await
            .map_err(|e| WeixinError::Unknown(format!("error request unifiedorder api: {}", e)))?;
        let res_text = res.text().await.map_err(|e| {
            WeixinError::Unknown(format!("error parse unifiedorder response: {}", e))
        })?;
        tracing::debug!("unifiedorder response: {:?}", res_text);

        let res_obj: WxJSAPIResponse = quick_xml::de::from_str(&res_text).map_err(|e| {
            WeixinError::MalformedPayload(format!("error deserialize WxJSAPIResponse: {}", e))
        })?;
        if res_obj.return_code != "SUCCESS" {
            return Err(WeixinError::Unknown(format!(
                "unifiedorder return_code != SUCCESS: {}",
                &res_obj.return_msg
            )));
        }
        if res_obj.result_code != Some("SUCCESS".to_string()) {
            return Err(WeixinError::Unknown(format!(
                "unifiedorder result_code != SUCCESS: {:?}",
                res_obj.err_code_des.as_ref()
            )));
        }

        /* paySign 不是用前面的 sign, 需要重新生成 */
        let mut res_json = json!({
            "appId": res_obj.appid,
            "timeStamp": chrono::Utc::now().timestamp().to_string(),
            "nonceStr": &v2_api_payload.nonce_str,
            "package": format!("prepay_id={}", res_obj.prepay_id.as_ref().unwrap_or(&"".to_string())),
            "signType": "MD5",
            "paySign": "",
        });
        let m: HashMap<String, String> = serde_json::from_value(res_json.to_owned()).unwrap();
        let signature = v2api::v2api_md5::sign(&m, &config.wx_pub_key);
        res_json["paySign"] = serde_json::Value::String(signature);

        Ok(res_json)
    }

    pub fn process_notify(
        config: WxPubConfig,
        payload: &str,
    ) -> Result<WeixinTradeStatus, WeixinError> {
        let notify_payload = V2ApiNotifyPayload::new(payload)?;
        let verified = notify_payload.verify_md5_sign(&config.wx_pub_key)?;
        if !verified {
            return Err(WeixinError::MalformedPayload("wrong md5 sign".into()));
        }
        Ok(notify_payload.status)
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
