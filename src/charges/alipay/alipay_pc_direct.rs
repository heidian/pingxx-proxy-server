use serde::{Deserialize, Serialize};
use serde_json::json;
use crate::charges::charge::CreateChargeRequestPayload;

/// 实现 Requester 接口的各种方法
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
    pub fn get_sorted_sign_source(&self) -> Result<String, ()> {
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

    pub fn create_credential(req_payload: &CreateChargeRequestPayload) -> Result<serde_json::Value, ()> {
        let mut alipay_pc_direct_req = AlipayPcDirectRequest {
            service: "create_direct_pay_by_user".to_string(),
            _input_charset: "utf-8".to_string(),
            return_url: "https://dxd1234.heidianer.com/order/be4570fbd7bf99d17f3b68589a5a46c2a7b302c8?payment_status=success".to_string(),
            notify_url: "https://notify.pingxx.com/notify/charges/ch_101240601691280343040013".to_string(),
            partner: "2088612364840749".to_string(),
            out_trade_no: req_payload.order_no.clone(),
            subject: req_payload.subject.to_string(),
            body: req_payload.body.to_string(),
            total_fee: "8.00".to_string(),
            payment_type: "1".to_string(),
            seller_id: "2088612364840749".to_string(),
            it_b_pay: "26m".to_string(),
            sign: "".to_string(),
            sign_type: "RSA".to_string()
        };

        let sign_sorted_source = alipay_pc_direct_req.get_sorted_sign_source().unwrap();
        tracing::info!("sign_source: {}", sign_sorted_source);
        let signture = sign(&sign_sorted_source).unwrap();
        tracing::info!("signture: {}", signture);
        alipay_pc_direct_req.sign = signture;
        Ok(json!(alipay_pc_direct_req))
    }
}


use openssl::hash::MessageDigest;
use openssl::pkey::PKey;
use openssl::rsa::Rsa;
use openssl::sign::Signer;
fn sign(sign_source: &str) -> Result<String, ()> {
    let pingxx_params = std::env::var("PINGXX_PARAMS").unwrap();
    let pingxx_params: serde_json::Value = serde_json::from_str(&pingxx_params).unwrap();
    let alipay_params = pingxx_params["alipay_pc_direct"].to_owned();
    // tracing::info!("alipay_params: {}", pingxx_params);
    let private_key = alipay_params["alipay_private_key"].as_str().unwrap();
    let keypair = Rsa::private_key_from_pem(private_key.as_bytes()).unwrap();
    let keypair = PKey::from_rsa(keypair).unwrap();
    let mut signer = Signer::new(MessageDigest::sha1(), &keypair).unwrap();
    signer.update(sign_source.as_bytes()).unwrap();
    let signature_bytes = signer.sign_to_vec().unwrap();
    let signature = data_encoding::BASE64.encode(&signature_bytes);
    Ok(signature)
}
