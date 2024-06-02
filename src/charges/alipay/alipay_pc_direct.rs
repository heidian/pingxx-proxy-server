use crate::charges::charge::CreateChargeRequestPayload;
use openssl::{hash::MessageDigest, pkey::PKey, rsa::Rsa, sign::Signer};
use serde::{Deserialize, Serialize};
use serde_json::json;

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct AlipayPcDirectConfig {
    pub alipay_pid: String,
    pub alipay_security_key: String,
    pub alipay_account: String,

    pub alipay_version: u32, // 1: mapi, 2: openapi
    pub alipay_app_id: String,

    pub alipay_sign_type: String, // RSA, RSA2
    pub alipay_private_key: String,
    pub alipay_public_key: String,
    pub alipay_private_key_rsa2: String,
    pub alipay_public_key_rsa2: String,
}

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct AlipayPcDirectRequest {
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

impl AlipayPcDirectRequest {
    fn get_sorted_sign_source(&self) -> Result<String, ()> {
        let v = serde_json::to_string(&self).map_err(|e| {
            tracing::error!("get_sorted_sign_source: {}", e);
        })?;
        let m: std::collections::HashMap<String, String> = serde_json::from_str(v.as_str())
            .map_err(|e| {
                tracing::error!("get_sorted_sign_source: {}", e);
            })?;
        let mut query_list = Vec::<String>::new();
        m.iter().for_each(|(k, v)| {
            if !v.is_empty() && k != "sign" && k != "sign_type" {
                let query = format!("{}={}", k, v.trim());
                query_list.push(query);
            }
        });
        query_list.sort();
        Ok(query_list.join("&"))
    }

    fn sign(sign_source: &str, private_key: &str) -> Result<String, ()> {
        let keypair = Rsa::private_key_from_pem(private_key.as_bytes()).unwrap();
        let keypair = PKey::from_rsa(keypair).unwrap();
        let mut signer = Signer::new(MessageDigest::sha1(), &keypair).unwrap();
        signer.update(sign_source.as_bytes()).unwrap();
        let signature_bytes = signer.sign_to_vec().unwrap();
        let signature = data_encoding::BASE64.encode(&signature_bytes);
        Ok(signature)
    }

    pub fn create_credential(
        req_payload: &CreateChargeRequestPayload,
        notify_url: &str,
    ) -> Result<serde_json::Value, ()> {
        let pingxx_params = std::env::var("PINGXX_PARAMS").unwrap();
        let pingxx_params: serde_json::Value = serde_json::from_str(&pingxx_params).unwrap();
        let config: AlipayPcDirectConfig =
            serde_json::from_value(pingxx_params["alipay_pc_direct"].to_owned()).unwrap();
        let returl_url = match req_payload.extra.success_url.as_ref() {
            Some(url) => url.to_string(),
            None => "".to_string(),
        };
        let total_fee = format!("{:.2}", req_payload.amount as f64 / 100.0);
        let it_b_pay = {
            let now = chrono::Utc::now().timestamp() as u32;
            if req_payload.time_expire > now {
                let seconds = req_payload.time_expire - now;
                format!("{}m", if seconds > 60 { seconds / 60 } else { 1 })
            } else {
                tracing::error!("create_credential: expire_in_seconds < now");
                return Err(());
            }
        };
        let mut alipay_pc_direct_req = AlipayPcDirectRequest {
            service: String::from("create_direct_pay_by_user"),
            _input_charset: String::from("utf-8"),
            return_url: returl_url,
            notify_url: notify_url.to_string(),
            partner: config.alipay_pid.clone(),
            out_trade_no: req_payload.order_no.clone(),
            subject: req_payload.subject.clone(),
            body: req_payload.body.clone(),
            total_fee: total_fee,
            payment_type: "1".to_string(),
            seller_id: config.alipay_pid.clone(),
            it_b_pay: it_b_pay,
            sign: "".to_string(),
            sign_type: "RSA".to_string(),
        };
        let sign_sorted_source = alipay_pc_direct_req.get_sorted_sign_source().unwrap();
        tracing::info!("sign_source: {}", sign_sorted_source);
        let signture =
            AlipayPcDirectRequest::sign(&sign_sorted_source, &config.alipay_private_key).unwrap();
        tracing::info!("signture: {}", signture);
        alipay_pc_direct_req.sign = signture;
        Ok(json!(alipay_pc_direct_req))
    }
}
