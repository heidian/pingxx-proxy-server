use openssl::{
    hash::MessageDigest,
    pkey::PKey,
    rsa::Rsa,
    sign::{Signer, Verifier},
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::str::FromStr;

use crate::charges::alipay::AlipayTradeStatus;

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
    fn get_sorted_sign_source(&self) -> String {
        // 这里 deserialize 不会出问题
        let v = serde_json::to_value(&self).unwrap();
        let m: HashMap<String, String> = serde_json::from_value(v).unwrap();
        let mut query_list = Vec::<String>::new();
        m.iter().for_each(|(k, v)| {
            if !v.is_empty() && k != "sign" && k != "sign_type" && k != "channel_url" {
                let query = format!("{}={}", k, v.trim());
                query_list.push(query);
            }
        });
        query_list.sort();
        query_list.join("&")
    }

    pub fn sign_rsa(&mut self, private_key: &str) -> Result<String, openssl::error::ErrorStack> {
        let sign_sorted_source = self.get_sorted_sign_source();
        tracing::info!("sign_source: {}", sign_sorted_source);
        let keypair = Rsa::private_key_from_pem(private_key.as_bytes())?;
        let keypair = PKey::from_rsa(keypair)?;
        let mut signer = Signer::new(MessageDigest::sha1(), &keypair)?;
        signer.update(sign_sorted_source.as_bytes())?;
        let signature_bytes = signer.sign_to_vec()?;
        let signature = data_encoding::BASE64.encode(&signature_bytes);
        tracing::info!("signture: {}", &signature);
        self.sign = signature.clone();
        Ok(signature)
    }
}

pub struct MapiNotifyPayload {
    pub trade_status: AlipayTradeStatus,
    pub out_trade_no: String,
    pub total_fee: String,
    signagure: String,
    m: HashMap<String, String>,
}

impl MapiNotifyPayload {
    /**
     * convert key1=value1&key2=value2 to HashMap
     * 先要进行一次处理把 x-www-form-urlencoded 数据中的 + 还原为空格
     * 主要是时间值比如 gmt_create=2024-06-09+18:07:41&xxx 要转换成 gmt_create=2024-06-09 18:07:41&xxx
     * 这个要放在 url decode 之前, 不然 decode 完了以后会出现新的 + 号 (比如 sign 里面, 那里的加号需要保留)
     */
    pub fn new(payload: &str) -> Result<Self, ()> {
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
        // tracing::debug!("m: {:?}", m);

        fn missing_params() {
            tracing::error!("openapi notify request missing required params");
        }

        let sign_type = m.get("sign_type").ok_or_else(missing_params)?;
        let signagure = m.get("sign").ok_or_else(missing_params)?;
        let trade_status = m.get("trade_status").ok_or_else(missing_params)?;
        let out_trade_no = m.get("out_trade_no").ok_or_else(missing_params)?;
        let total_fee = m.get("total_fee").ok_or_else(missing_params)?;

        if sign_type != "RSA" {
            tracing::error!("sign_type not RSA");
            return Err(());
        }

        let trade_status = AlipayTradeStatus::from_str(trade_status).map_err(|_| {})?;

        Ok(MapiNotifyPayload {
            trade_status: trade_status,
            out_trade_no: out_trade_no.to_owned(),
            total_fee: total_fee.to_owned(),
            signagure: signagure.to_owned(),
            m,
        })
    }

    pub fn verify_rsa_sign(&self, public_key: &str) -> Result<bool, openssl::error::ErrorStack> {
        let mut query_list = Vec::<String>::new();
        self.m.iter().for_each(|(k, v)| {
            if !v.is_empty() && k != "sign" && k != "sign_type" {
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
        let signature_bytes = data_encoding::BASE64
            .decode(self.signagure.as_bytes())
            .unwrap_or_default();
        let result = verifier.verify(&signature_bytes)?;
        // tracing::debug!("verify result: {}", result);

        Ok(result)
    }
}
