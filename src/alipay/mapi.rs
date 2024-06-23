use super::AlipayError;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

mod mapi_rsa {
    use super::*;
    use openssl::{
        hash::MessageDigest,
        pkey::PKey,
        rsa::Rsa,
        sign::{Signer, Verifier},
    };

    pub fn sign_md5(m: &HashMap<String, String>, sign_key: &str) -> Result<String, AlipayError> {
        let mut query_list = Vec::<String>::new();
        m.iter().for_each(|(k, v)| {
            if !v.is_empty() {
                let query = format!("{}={}", k, v.trim());
                query_list.push(query);
            }
        });
        query_list.sort();
        let sign_sorted_source = format!("{}{}", query_list.join("&"), sign_key);
        let signature = md5::compute(sign_sorted_source.as_bytes());
        let signature = format!("{:x}", signature); // .to_uppercase();
        Ok(signature)
    }

    pub fn sign(m: &HashMap<String, String>, private_key: &str) -> Result<String, AlipayError> {
        let mut query_list = Vec::<String>::new();
        m.iter().for_each(|(k, v)| {
            if !v.is_empty() {
                let query = format!("{}={}", k, v.trim());
                query_list.push(query);
            }
        });
        query_list.sort();
        let sign_sorted_source = query_list.join("&");
        let keypair = Rsa::private_key_from_pem(private_key.as_bytes())?;
        let keypair = PKey::from_rsa(keypair)?;
        let mut signer = Signer::new(MessageDigest::sha1(), &keypair)?;
        signer.update(sign_sorted_source.as_bytes())?;
        let signature_bytes = signer.sign_to_vec()?;
        let signature = data_encoding::BASE64.encode(&signature_bytes);
        Ok(signature)
    }

    pub fn verify(
        m: &HashMap<String, String>,
        signature: &str,
        public_key: &str,
    ) -> Result<bool, AlipayError> {
        let mut query_list = Vec::<String>::new();
        m.iter().for_each(|(k, v)| {
            if !v.is_empty() {
                let query = format!("{}={}", k, v);
                query_list.push(query);
            }
        });
        query_list.sort();
        let sorted_payload = query_list.join("&");
        let keypair = Rsa::public_key_from_pem(public_key.as_bytes())?;
        let keypair = PKey::from_rsa(keypair)?;
        let mut verifier = Verifier::new(MessageDigest::sha1(), &keypair)?;
        verifier.update(sorted_payload.as_bytes())?;
        let signature_bytes = data_encoding::BASE64.decode(signature.as_bytes())?;
        let result = verifier.verify(&signature_bytes)?;
        Ok(result)
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct MapiRequestPayload {
    pub channel_url: String,
    pub service: String,
    pub _input_charset: String,
    pub return_url: String,
    pub notify_url: String,
    pub partner: String,
    pub out_trade_no: String,
    pub subject: String,
    pub body: String,
    pub total_fee: String,
    pub payment_type: String,
    pub seller_id: String,
    pub it_b_pay: String,
    pub sign: String,
    pub sign_type: String,
}

impl MapiRequestPayload {
    pub fn new(
        charge_id: &str,         //
        service: &str,           // create_direct_pay_by_user | alipay.wap.create.direct.pay.by.user
        alipay_pid: &str,        // 合作者身份 ID, 商家唯一 ID
        return_url: &str,        // 支付成功跳转
        merchant_order_no: &str, // 商户订单号
        charge_amount: i32,      // 支付金额, 精确到分
        time_expire: i32,        // 过期时间 timestamp 精确到秒
        subject: &str,           // 标题
        body: &str,              // 详情
    ) -> Result<Self, AlipayError> {
        let total_fee = format!("{:.2}", charge_amount as f64 / 100.0);
        let it_b_pay = {
            let now = chrono::Utc::now().timestamp() as i32;
            if time_expire > now {
                let seconds = time_expire - now;
                format!("{}m", if seconds > 60 { seconds / 60 } else { 1 })
            } else {
                return Err(AlipayError::MalformedRequest(
                    "expire_in_seconds < now".into(),
                ));
            }
        };
        let payload = Self {
            channel_url: String::from("https://mapi.alipay.com/gateway.do"),
            service: String::from(service),
            _input_charset: String::from("utf-8"),
            return_url: return_url.to_string(),
            notify_url: crate::utils::notify_url(charge_id),
            partner: alipay_pid.to_string(),
            out_trade_no: merchant_order_no.to_string(),
            subject: subject.to_string(),
            body: body.to_string(),
            total_fee,
            payment_type: String::from("1"),
            seller_id: alipay_pid.to_string(),
            it_b_pay,
            sign: String::from(""),
            sign_type: String::from("RSA"),
        };
        Ok(payload)
    }

    pub fn sign_rsa(&mut self, private_key: &str) -> Result<String, AlipayError> {
        let v = serde_json::to_value(&self).unwrap();
        let mut m: HashMap<String, String> = serde_json::from_value(v).unwrap();
        m.remove("sign");
        m.remove("sign_type");
        m.remove("channel_url");
        let signature = mapi_rsa::sign(&m, private_key)?;
        self.sign = signature.clone();
        Ok(signature)
    }
}

pub struct MapiNotifyPayload {
    pub trade_status: String,
    pub merchant_order_no: String,
    pub amount: i32,
    signature: String,
    m: HashMap<String, String>,
}

impl MapiNotifyPayload {
    /**
     * convert key1=value1&key2=value2 to HashMap
     * 先要进行一次处理把 x-www-form-urlencoded 数据中的 + 还原为空格
     * 主要是时间值比如 gmt_create=2024-06-09+18:07:41&xxx 要转换成 gmt_create=2024-06-09 18:07:41&xxx
     * 这个要放在 url decode 之前, 不然 decode 完了以后会出现新的 + 号 (比如 sign 里面, 那里的加号需要保留)
     */
    pub fn new(payload: &str) -> Result<Self, AlipayError> {
        let payload = payload.replace("+", " ");
        let mut m: HashMap<String, String> = HashMap::new();
        payload.split('&').for_each(|pair| {
            let kv: Vec<&str> = pair.split('=').collect();
            if kv.len() == 2 {
                let key = kv[0].to_string();
                let val = percent_encoding::percent_decode_str(kv[1])
                    .decode_utf8()
                    .unwrap_or_default()
                    .to_string();
                m.insert(key, val);
            }
        });

        fn missing_params() -> AlipayError {
            AlipayError::ApiError("missing required params".into())
        }

        let sign_type = m.get("sign_type").ok_or_else(missing_params)?;
        let signature = m.get("sign").ok_or_else(missing_params)?;
        let trade_status = m.get("trade_status").ok_or_else(missing_params)?;
        let out_trade_no = m.get("out_trade_no").ok_or_else(missing_params)?;
        let total_fee = m.get("total_fee").ok_or_else(missing_params)?;

        if sign_type != "RSA" {
            return Err(AlipayError::ApiError("sign_type not RSA".into()));
        }

        let amount = (total_fee
            .parse::<f64>()
            .map_err(|_| AlipayError::ApiError("invalid total_fee".into()))?
            * 100.0) as i32;

        Ok(Self {
            trade_status: trade_status.to_owned(),
            merchant_order_no: out_trade_no.to_owned(),
            amount,
            signature: signature.to_owned(),
            m,
        })
    }

    pub fn verify_rsa_sign(&self, public_key: &str) -> Result<(), AlipayError> {
        let mut m = self.m.clone();
        // k != "sign" && k != "sign_type";
        m.remove("sign_type");
        m.remove("sign");
        let verified = mapi_rsa::verify(&m, &self.signature, public_key)?;
        if !verified {
            return Err(AlipayError::ApiError("wrong rsa signature".into()));
        }
        Ok(())
    }
}

#[derive(Debug, Serialize)]
pub struct MapiRefundPayload {
    pub service: String,
    pub partner: String,
    pub _input_charset: String,
    pub sign_type: String,
    pub sign: String,
    pub notify_url: String,
    // pub seller_email: String,
    pub seller_user_id: String,
    pub refund_date: String,
    pub batch_no: String,
    pub batch_num: String,
    pub detail_data: String,
}

impl MapiRefundPayload {
    pub fn new(
        refund_id: &str,
        alipay_pid: &str,        // 合作者身份 ID, 商家唯一 ID
        // alipay_account: &str,   // 支付宝账号
        merchant_order_no: &str, // 商户订单号
        refund_amount: i32,      // 退款金额, 精确到分
        description: &str,       // 退款说明
    ) -> Result<Self, AlipayError> {
        let refund_amount = format!("{:.2}", refund_amount as f64 / 100.0);
        let now = chrono::Utc::now();
        let batch_no = format!(
            "{}{}",
            now.format("%Y%m%d").to_string(),
            now.timestamp_millis().to_string()
        );
        let refund_date = now.format("%Y-%m-%d %H:%M:%S").to_string();
        Ok(Self {
            service: String::from("refund_fastpay_by_platform_pwd"),
            partner: alipay_pid.to_string(),
            _input_charset: String::from("utf-8"),
            sign_type: String::from("RSA"),
            sign: String::from(""),
            notify_url: crate::utils::refund_notify_url(refund_id),
            // seller_email: alipay_account.to_string(),
            seller_user_id: alipay_pid.to_string(),
            refund_date: refund_date.clone(),
            batch_no: batch_no.clone(),
            batch_num: String::from("1"),
            detail_data: format!("{}^{}^{}", merchant_order_no, refund_amount, description),
        })
    }

    pub fn sign_rsa(&mut self, private_key: &str) -> Result<String, AlipayError> {
        // 这里 deserialize 不会出问题
        let v = serde_json::to_value(&self).unwrap();
        let mut m: HashMap<String, String> = serde_json::from_value(v).unwrap();
        m.remove("sign");
        m.remove("sign_type");
        let signature = mapi_rsa::sign(&m, private_key)?;
        self.sign = signature.clone();
        Ok(signature)
    }

    #[allow(dead_code)]
    pub fn sign_md5(&mut self, sign_key: &str) -> Result<String, AlipayError> {
        // 这里 deserialize 不会出问题
        let v = serde_json::to_value(&self).unwrap();
        let mut m: HashMap<String, String> = serde_json::from_value(v).unwrap();
        m.remove("sign");
        m.remove("sign_type");
        let signature = mapi_rsa::sign_md5(&m, sign_key)?;
        self.sign = signature.clone();
        Ok(signature)
    }

    pub async fn build_refund_url(&self) -> Result<String, AlipayError> {
        let res = reqwest::Client::new()
            .get("https://mapi.alipay.com/gateway.do")
            .query(&self)
            .build()
            .map_err(|e| AlipayError::Unexpected(format!("error building refund url: {:?}", e)))?;
        let url = res.url().to_string();
        // tracing::debug!("alipay mapi refund url: {}", url);
        Ok(url)
    }
}
