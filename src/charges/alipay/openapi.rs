use openssl::{hash::MessageDigest, pkey::PKey, rsa::Rsa, sign::Signer, sign::Verifier};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Serialize, Deserialize)]
pub struct OpenApiRequestPayload {
    pub app_id: String,
    pub method: String,
    pub format: String,
    pub charset: String,
    pub sign_type: String,
    pub timestamp: String,
    pub version: String,
    pub biz_content: String,
    pub notify_url: String,
    pub return_url: String,
    pub sign: String,
    pub channel_url: String,
}

impl OpenApiRequestPayload {
    fn get_sorted_sign_source(&self) -> String {
        // 这里 deserialize 不会出问题
        let v = serde_json::to_value(&self).unwrap();
        let m: HashMap<String, String> = serde_json::from_value(v).unwrap();
        let mut query_list = Vec::<String>::new();
        m.iter().for_each(|(k, v)| {
            if !v.is_empty() && k != "sign" && k != "channel_url" {
                let query = format!("{}={}", k, v.trim());
                query_list.push(query);
            }
        });
        query_list.sort();
        query_list.join("&")
    }

    pub fn sign_rsa2(&mut self, private_key: &str) -> Result<String, openssl::error::ErrorStack> {
        let sign_sorted_source = self.get_sorted_sign_source();
        tracing::info!("sign_source: {}", sign_sorted_source);
        let keypair = Rsa::private_key_from_pem(private_key.as_bytes())?;
        let keypair = PKey::from_rsa(keypair)?;
        let mut signer = Signer::new(MessageDigest::sha256(), &keypair)?;
        signer.update(sign_sorted_source.as_bytes())?;
        let signature_bytes = signer.sign_to_vec()?;
        let signature = data_encoding::BASE64.encode(&signature_bytes);
        tracing::info!("signture: {}", &signature);
        self.sign = signature.clone();
        Ok(signature)
    }
}

pub fn verify_rsa2_sign(
    payload: &str,
    public_key: &str,
) -> Result<bool, openssl::error::ErrorStack> {
    tracing::info!(
        payload = payload,
        public_key = public_key,
        "verify signature"
    );

    // convert key1=value1&key2=value2 to HashMap
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

    let _sign_type = match m.get("sign_type") {
        Some(sign_type) => {
            if sign_type != "RSA2" {
                tracing::error!("sign_type not RSA2");
                return Ok(false);
            } else {
                sign_type.to_string()
            }
        }
        None => {
            tracing::error!("sign_type not found");
            return Ok(false);
        }
    };

    let signagure = match m.get("sign") {
        Some(sign) => sign.to_string(),
        None => {
            tracing::error!("sign not found");
            return Ok(false);
        }
    };

    let mut query_list = Vec::<String>::new();
    m.iter().for_each(|(k, v)| {
        if !v.is_empty() && k != "sign" && k != "sign_type" {
            let v = v.trim();
            // 主要是时间值比如 2024-06-09+18:07:41 要转换成 2024-06-09 18:07:41
            let v = v.replace("+", " ");
            let query = format!("{}={}", k, v);
            query_list.push(query);
        }
    });
    query_list.sort();
    let sorted_payload = query_list.join("&");

    let keypair = Rsa::public_key_from_pem(public_key.as_bytes())?;
    let keypair = PKey::from_rsa(keypair)?;
    let mut verifier = Verifier::new(MessageDigest::sha256(), &keypair)?;
    verifier.update(sorted_payload.as_bytes())?;
    let signature_bytes = data_encoding::BASE64
        .decode(signagure.as_bytes())
        .unwrap_or_default();
    let result = verifier.verify(&signature_bytes)?;
    // tracing::debug!("verify result: {}", result);

    Ok(result)
}
