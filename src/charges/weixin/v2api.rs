use super::config::{WeixinError, WeixinTradeStatus};
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, str::FromStr};

pub mod v2api_md5 {
    use std::collections::HashMap;
    /**
     * https://pay.weixin.qq.com/wiki/doc/api/jsapi.php?chapter=4_3
     */
    pub fn sign(m: &HashMap<String, String>, sign_key: &str) -> String {
        let mut query_list = Vec::<String>::new();
        m.iter().for_each(|(k, v)| {
            if !v.is_empty() {
                // 不需要 加 && k != "sign" && k != "paySign" 因为 sign 之前他们是 ""
                let query = format!("{}={}", k, v.trim());
                query_list.push(query);
            }
        });
        query_list.sort();
        let sign_sorted_source = format!("{}&key={}", query_list.join("&"), sign_key);
        let signature = md5::compute(sign_sorted_source.as_bytes());
        let signature = format!("{:x}", signature).to_uppercase();
        signature
    }

    pub fn verify(m: &HashMap<String, String>, signature: &str, sign_key: &str) -> bool {
        let mut query_list = Vec::<String>::new();
        m.iter().for_each(|(k, v)| {
            if !v.is_empty() {
                let query = format!("{}={}", k, v.trim());
                query_list.push(query);
            }
        });
        query_list.sort();
        let sign_sorted_source = format!("{}&key={}", query_list.join("&"), sign_key);
        let sign = md5::compute(sign_sorted_source.as_bytes());
        let sign = format!("{:x}", sign).to_uppercase();
        sign == *signature
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct V2ApiRequestPayload {
    pub appid: String,
    pub mch_id: String,
    pub nonce_str: String,
    pub sign: String,
    // pub sign_type: String,
    pub body: String,
    pub out_trade_no: String,
    pub total_fee: String,
    pub spbill_create_ip: String,
    pub time_expire: String,
    pub notify_url: String,
    pub trade_type: String,
    pub openid: String,
}

impl V2ApiRequestPayload {
    pub fn new(
        _charge_id: &str,        //
        wx_pub_app_id: &str,     // 微信公众号 app id
        wx_pub_mch_id: &str,     // 微信支付商户 id
        open_id: &str,           // 支付成功跳转
        client_ip: &str,         // 客户端 IP
        notify_url: &str,        // 异步通知
        merchant_order_no: &str, // 商户订单号
        charge_amount: i32,      // 支付金额, 精确到分
        time_expire: i32,        // 过期时间 timestamp 精确到秒
        _subject: &str,          // 标题
        body: &str,              // 详情
    ) -> Result<Self, WeixinError> {
        let time_expire = chrono::DateTime::<chrono::Utc>::from_timestamp(time_expire as i64, 0)
            .ok_or_else(|| WeixinError::MalformedPayload("can't convert timestamp to datetime".into()))?
            .with_timezone(&chrono::FixedOffset::east_opt(8 * 3600).unwrap())
            .format("%Y%m%d%H%M%S")
            .to_string();
        let total_fee = format!("{}", charge_amount);
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
        let payload = V2ApiRequestPayload {
            appid: wx_pub_app_id.to_string(),
            mch_id: wx_pub_mch_id.to_string(),
            nonce_str,
            sign: String::from(""),
            // sign_type: "MD5",
            body: body.to_string(),
            out_trade_no: merchant_order_no.to_string(),
            total_fee,
            spbill_create_ip: client_ip.to_string(),
            time_expire,
            notify_url: notify_url.to_string(),
            trade_type: String::from("JSAPI"),
            openid: open_id.to_string(),
        };
        Ok(payload)
    }

    /**
     * https://pay.weixin.qq.com/wiki/doc/api/jsapi.php?chapter=4_3
     */
    pub fn sign_md5(&mut self, sign_key: &str) -> Result<String, WeixinError> {
        // 这里 deserialize 不会出问题
        let v = serde_json::to_value(&self).unwrap();
        let m: HashMap<String, String> = serde_json::from_value(v.to_owned()).unwrap();
        let signature = v2api_md5::sign(&m, sign_key);
        self.sign = signature.clone();
        Ok(signature)
    }
}

pub struct V2ApiNotifyPayload {
    pub status: WeixinTradeStatus,
    pub merchant_order_no: String,
    pub amount: i32,
    signature: String,
    m: HashMap<String, String>,
}

impl V2ApiNotifyPayload {
    pub fn new(payload: &str) -> Result<Self, WeixinError> {
        let mut m = HashMap::<String, String>::new();
        let mut parser = quick_xml::Reader::from_str(payload);
        parser.config_mut().trim_text(true);
        let _ = parser.read_event(); // Skip root element
        loop {
            match parser.read_event() {
                Ok(quick_xml::events::Event::Start(ref e)) => {
                    let key = String::from_utf8(e.name().0.to_vec()).unwrap();
                    // let value = parser.read_text(e.name()).unwrap();
                    // m.insert(key, value.as_ref().to_owned());
                    let value = match parser.read_event() {
                        Ok(quick_xml::events::Event::CData(cdata)) => {
                            String::from_utf8(cdata.to_vec()).unwrap()
                        }
                        Ok(quick_xml::events::Event::Text(text)) => {
                            text.unescape().unwrap().to_string()
                        }
                        _ => String::new(),
                    };
                    m.insert(key, value);
                }
                Ok(quick_xml::events::Event::Eof) => break,
                Err(e) => {
                    return Err(WeixinError::MalformedPayload(format!(
                        "error parsing xml {}",
                        e
                    )))
                }
                _ => {}
            }
        }

        if m.get("return_code") != Some(&"SUCCESS".to_string()) {
            return Err(WeixinError::MalformedPayload(
                "return_code not SUCCESS".into(),
            ));
        }

        fn missing_params() -> WeixinError {
            WeixinError::MalformedPayload("missing required params".into())
        }

        let signature = m.get("signature").ok_or_else(missing_params)?;
        let trade_status = m.get("trade_status").ok_or_else(missing_params)?;
        let out_trade_no = m.get("out_trade_no").ok_or_else(missing_params)?;
        let total_fee = m.get("total_fee").ok_or_else(missing_params)?;

        let trade_status = WeixinTradeStatus::from_str(trade_status)
            .map_err(|_| WeixinError::MalformedPayload("unknown trade_status".into()))?;

        let amount = (total_fee
            .parse::<f64>()
            .map_err(|_| WeixinError::MalformedPayload("invalid total_fee".into()))?
            * 100.0) as i32;

        Ok(Self {
            status: trade_status,
            merchant_order_no: out_trade_no.to_owned(),
            amount,
            signature: signature.to_owned(),
            m,
        })
    }

    pub fn verify_md5_sign(&self, public_key: &str) -> Result<bool, WeixinError> {
        let mut m = self.m.clone();
        // k != "sign";
        m.remove("sign");
        let verified = v2api_md5::verify(&self.m, &self.signature, public_key);
        Ok(verified)
    }
}
