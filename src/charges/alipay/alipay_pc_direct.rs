use crate::charges::{
    charge::CreateChargeRequestPayload,
    //
};
use openssl::{hash::MessageDigest, pkey::PKey, rsa::Rsa, sign::Signer};
use serde::{Deserialize, Serialize};
use serde_json::json;

#[derive(Debug)]
enum AlipayApiType {
    MAPI,
    OPENAPI,
}

impl<'de> Deserialize<'de> for AlipayApiType {
    fn deserialize<D>(deserializer: D) -> Result<AlipayApiType, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let s = i32::deserialize(deserializer)?;
        match s {
            1 => Ok(AlipayApiType::MAPI),
            2 => Ok(AlipayApiType::OPENAPI),
            _ => Err(serde::de::Error::custom(format!(
                "unknown alipay_api_type: {}",
                s
            ))),
        }
    }
}

#[derive(Debug, Deserialize)]
enum AlipaySignType {
    #[serde(rename = "rsa")]
    RSA,
    #[serde(rename = "rsa2")]
    RSA256,
}

#[allow(dead_code)]
#[derive(Debug, Deserialize)]
struct AlipayPcDirectConfig {
    alipay_pid: String,
    alipay_security_key: String,
    alipay_account: String,

    alipay_version: AlipayApiType,
    alipay_app_id: String,

    alipay_sign_type: AlipaySignType,
    alipay_private_key: String,
    alipay_public_key: String,
    alipay_private_key_rsa2: String,
    alipay_public_key_rsa2: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct AlipayPcDirectMapiRequest {
    service: String,
    _input_charset: String,
    return_url: String,
    notify_url: String,
    partner: String,
    out_trade_no: String,
    subject: String,
    body: String,
    total_fee: String,
    payment_type: String,
    seller_id: String,
    it_b_pay: String,
    sign: String,
    sign_type: String,
}

impl AlipayPcDirectMapiRequest {
    fn get_sorted_sign_source(&self) -> String {
        // 这里 deserialize 不会出问题
        let v = serde_json::to_value(&self).unwrap();
        let m: std::collections::HashMap<String, String> = serde_json::from_value(v).unwrap();
        let mut query_list = Vec::<String>::new();
        m.iter().for_each(|(k, v)| {
            if !v.is_empty() && k != "sign" && k != "sign_type" {
                let query = format!("{}={}", k, v.trim());
                query_list.push(query);
            }
        });
        query_list.sort();
        query_list.join("&")
    }

    fn sign_rsa(&mut self, private_key: &str) -> Result<String, openssl::error::ErrorStack> {
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

#[derive(Debug, Serialize, Deserialize)]
struct AlipayPcDirectOpenApiRequest {
    app_id: String,
    method: String,
    format: String,
    charset: String,
    sign_type: String,
    timestamp: String,
    version: String,
    biz_content: String,
    notify_url: String,
    return_url: String,
    sign: String,
    channel_url: String,
}

impl AlipayPcDirectOpenApiRequest {
    fn get_sorted_sign_source(&self) -> String {
        // 这里 deserialize 不会出问题
        let v = serde_json::to_value(&self).unwrap();
        let m: std::collections::HashMap<String, String> = serde_json::from_value(v).unwrap();
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

    fn sign_rsa2(&mut self, private_key: &str) -> Result<String, openssl::error::ErrorStack> {
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

pub struct AlipayPcDirect {}

impl AlipayPcDirect {
    fn load_config() -> Result<AlipayPcDirectConfig, serde_json::error::Error> {
        let pingxx_params = std::env::var("PINGXX_PARAMS").unwrap_or_default();
        let pingxx_params: serde_json::Value = serde_json::from_str(&pingxx_params)?;
        let config: AlipayPcDirectConfig =
            serde_json::from_value(pingxx_params["alipay_pc_direct"].to_owned())?;
        Ok(config)
    }

    fn create_mapi_credential(
        req_payload: &CreateChargeRequestPayload,
        notify_url: &str,
        config: AlipayPcDirectConfig,
    ) -> Result<serde_json::Value, ()> {
        let return_url = match req_payload.extra.success_url.as_ref() {
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
        let mut alipay_pc_direct_req = AlipayPcDirectMapiRequest {
            service: String::from("create_direct_pay_by_user"),
            _input_charset: String::from("utf-8"),
            return_url,
            notify_url: notify_url.to_string(),
            partner: config.alipay_pid.clone(),
            out_trade_no: req_payload.order_no.clone(),
            subject: req_payload.subject.clone(),
            body: req_payload.body.clone(),
            total_fee,
            payment_type: String::from("1"),
            seller_id: config.alipay_pid.clone(),
            it_b_pay,
            sign: String::from(""),
            sign_type: String::from("RSA"),
        };
        alipay_pc_direct_req
            .sign_rsa(&config.alipay_private_key)
            .map_err(|e| {
                tracing::error!("create_credential: {}", e);
            })?;
        Ok(json!(alipay_pc_direct_req))
    }

    fn create_openapi_credential(
        req_payload: &CreateChargeRequestPayload,
        notify_url: &str,
        config: AlipayPcDirectConfig,
    ) -> Result<serde_json::Value, ()> {
        let return_url = match req_payload.extra.success_url.as_ref() {
            Some(url) => url.to_string(),
            None => "".to_string(),
        };
        let total_amount = format!("{:.2}", req_payload.amount as f64 / 100.0);
        let timeout_express = {
            let now = chrono::Utc::now().timestamp() as u32;
            if req_payload.time_expire > now {
                let seconds = req_payload.time_expire - now;
                format!("{}m", if seconds > 60 { seconds / 60 } else { 1 })
            } else {
                tracing::error!("create_credential: expire_in_seconds < now");
                return Err(());
            }
        };
        let biz_content = json!({
            "body": req_payload.body.clone(),
            "subject": req_payload.subject.clone(),
            "out_trade_no": req_payload.order_no.clone(),
            "total_amount": total_amount,
            "product_code": "FAST_INSTANT_TRADE_PAY",
            "extend_params": { "sys_service_provider_id": config.alipay_pid.clone() },
            "timeout_express": timeout_express,
            "passback_params": "ch_101240602725900042240014"  // TODO: 这里要换成 charge id
        });
        let mut alipay_pc_direct_req = AlipayPcDirectOpenApiRequest {
            app_id: config.alipay_app_id.clone(),
            method: String::from("alipay.trade.page.pay"),
            format: String::from("JSON"),
            charset: String::from("utf-8"),
            sign_type: String::from("RSA2"),
            timestamp: chrono::Utc::now().format("%Y-%m-%d %H:%M:%S").to_string(),
            version: String::from("1.0"),
            biz_content: biz_content.to_string(),
            // "biz_content": "{\"body\":\"testproduct123-体积:30ML,重量:3mg\",\"subject\":\"鬼骨孖的店铺\",\"out_trade_no\":\"85020240602194128029\",\"total_amount\":\"11.00\",\"product_code\":\"FAST_INSTANT_TRADE_PAY\",\"extend_params\":{\"sys_service_provider_id\":\"2088421557811318\"},\"timeout_express\":\"30m\",\"passback_params\":\"ch_101240602725900042240014\"}",
            return_url,
            notify_url: notify_url.to_string(),
            sign: String::from(""),
            channel_url: String::from("https://openapi.alipay.com/gateway.do"),
        };
        alipay_pc_direct_req
            .sign_rsa2(&config.alipay_private_key_rsa2)
            .map_err(|e| {
                tracing::error!("create_credential: {}", e);
            })?;
        Ok(json!(alipay_pc_direct_req))
    }

    pub fn create_credential(
        req_payload: &CreateChargeRequestPayload,
        notify_url: &str,
    ) -> Result<serde_json::Value, ()> {
        let config = AlipayPcDirect::load_config().map_err(|e| {
            tracing::error!("error loading alipay_pc_direct config: {}", e);
        })?;
        match config.alipay_version {
            AlipayApiType::MAPI => {
                AlipayPcDirect::create_mapi_credential(req_payload, notify_url, config)
            }
            AlipayApiType::OPENAPI => {
                AlipayPcDirect::create_openapi_credential(req_payload, notify_url, config)
            }
        }
    }
}
