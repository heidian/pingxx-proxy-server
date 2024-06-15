use super::super::charge::CreateChargeRequestPayload;
use super::config::WxPubConfig;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::HashMap;

#[derive(Deserialize, Serialize, Debug)]
struct WxJSAPIResponse {
    return_code: String,
    return_msg: String,

    #[serde(skip_serializing_if = "Option::is_none")]
    appid: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    mch_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    nonce_str: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    sign: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    result_code: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    err_code: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    err_code_des: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    trade_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    prepay_id: Option<String>,
}

pub struct WxPub {}

impl WxPub {
    /**
     * https://pay.weixin.qq.com/wiki/doc/api/jsapi.php?chapter=4_3
     */
    fn create_sign(v: &serde_json::Value, sign_key: &str) -> String {
        let m: HashMap<String, String> = serde_json::from_value(v.to_owned()).unwrap();
        let mut query_list = Vec::<String>::new();
        m.iter().for_each(|(k, v)| {
            if !v.is_empty() && k != "sign" {
                let query = format!("{}={}", k, v.trim());
                query_list.push(query);
            }
        });
        query_list.sort();
        let sign_sorted_source = format!("{}&key={}", query_list.join("&"), sign_key);
        tracing::debug!("wx jsapi sign source: {}", sign_sorted_source);
        let sign = md5::compute(sign_sorted_source.as_bytes());
        let sign = format!("{:x}", sign).to_uppercase();
        tracing::debug!("wx jsapi sign: {}", sign);
        sign
    }

    /**
     * https://pay.weixin.qq.com/wiki/doc/api/jsapi.php?chapter=7_7&index=6
     * https://pay.weixin.qq.com/wiki/doc/api/jsapi.php?chapter=9_1
     */
    pub async fn create_credential(
        config: WxPubConfig,
        order: &crate::prisma::order::Data,
        charge_req_payload: &CreateChargeRequestPayload,
        notify_url: &str,
    ) -> Result<serde_json::Value, ()> {
        // convert timestamp (in seconds) to yyyyMMddHHmmss, timestamp is stored in order.time_expire
        let time_expire =
            chrono::DateTime::<chrono::Utc>::from_timestamp(order.time_expire as i64, 0)
                .ok_or_else(|| tracing::error!("convert timestamp to datetime"))?
                .with_timezone(&chrono::FixedOffset::east_opt(8 * 3600).unwrap())
                .format("%Y%m%d%H%M%S")
                .to_string();
        let total_fee = format!("{}", charge_req_payload.charge_amount);
        let open_id = match charge_req_payload.extra.open_id.as_ref() {
            Some(open_id) => open_id.to_string(),
            None => "".to_string(),
        };
        // create 32 charactors nonce string
        let nonce_str = (0..32)
            .map(|_| {
                let idx = rand::random::<usize>() % 62;
                if idx < 10 {
                    (idx as u8 + 48) as char
                } else if idx < 36 {
                    (idx as u8 + 55) as char
                } else {
                    (idx as u8 + 61) as char
                }
            })
            .collect::<String>();
        let mut payload = json!({
            "appid": &config.wx_pub_app_id,
            "mch_id": &config.wx_pub_mch_id,
            "nonce_str": &nonce_str,
            "sign": "",
            // "sign_type": "MD5",
            "body": &order.body,
            "out_trade_no": &order.merchant_order_no,
            "total_fee": &total_fee,
            "spbill_create_ip": &order.client_ip,
            "time_expire": &time_expire,
            "notify_url": notify_url,
            "trade_type": "JSAPI",
            "openid": &open_id,
        });
        let sign = Self::create_sign(&payload, &config.wx_pub_key);
        payload["sign"] = sign.into();

        let xml_payload = quick_xml::se::to_string_with_root("xml", &payload).map_err(|e| {
            tracing::error!("xml payload: {}", e);
        })?;
        tracing::debug!("wx jsapi xml payload: {}", xml_payload);

        let res = reqwest::Client::new()
            .post("https://api.mch.weixin.qq.com/pay/unifiedorder")
            .body(xml_payload)
            .send()
            .await
            .map_err(|e| tracing::error!("request error: {}", e))?;
        let res_text = res
            .text()
            .await
            .map_err(|e| tracing::error!("error parsing response: {}", e))?;
        tracing::debug!("wx jsapi response: {:?}", res_text);

        let res_json = {
            let res_obj: WxJSAPIResponse = quick_xml::de::from_str(&res_text).map_err(|e| {
                tracing::error!("error parsing WxJSAPIResponse: {}", e);
            })?;
            tracing::debug!("WxJSAPIResponse: {:?}", &res_obj);
            if res_obj.return_code != "SUCCESS" {
                tracing::error!("wx jsapi response error: {}", &res_obj.return_msg);
                return Err(());
            }
            if res_obj.result_code != Some("SUCCESS".to_string()) {
                tracing::error!(
                    "wx jsapi response error: {}",
                    res_obj.err_code_des.as_ref().unwrap()
                );
                return Err(());
            }
            /* paySign 不是用前面的 sign, 需要重新生成 */
            let mut res_json = json!({
                "appId": res_obj.appid,
                "timeStamp": chrono::Utc::now().timestamp().to_string(),
                "nonceStr": &nonce_str,
                "package": format!("prepay_id={}", res_obj.prepay_id.as_ref().unwrap_or(&"".to_string())),
                "signType": "MD5",
                "paySign": "",
            });
            let sign = Self::create_sign(&res_json, &config.wx_pub_key);
            res_json["paySign"] = serde_json::Value::String(sign);
            res_json
        };

        Ok(res_json)
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
