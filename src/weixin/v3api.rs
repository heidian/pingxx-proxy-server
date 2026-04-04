use super::WeixinError;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// ============================================================
// V2 API (MD5 + XML) — 原有逻辑，保持不变
// ============================================================

pub mod v2api_md5 {
    use super::*;
    /**
     * https://pay.weixin.qq.com/wiki/doc/api/jsapi.php?chapter=4_3
     * 对 m 的所有字段进行签名, 所以 m 里面不能包含不需要签名的字段比如 sign, paySign, 或者他们需要为空
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

    pub fn decrypt_aes256_ecb(text: &str, sign_key: &str) -> Result<String, String> {
        let text = data_encoding::BASE64
            .decode(text.as_bytes())
            .map_err(|e| format!("error decoding base64: {}", e))?;
        let sign_key_md5 = md5::compute(sign_key.as_bytes());
        let sign_key_md5 = format!("{:x}", sign_key_md5).to_lowercase();
        let cipher = openssl::symm::Cipher::aes_256_ecb();
        let decrypted = openssl::symm::decrypt(cipher, sign_key_md5.as_bytes(), None, &text)
            .map_err(|e| format!("error decrypting aes256 ecb: {}", e))?;
        let decrypted_text = String::from_utf8(decrypted)
            .map_err(|e| format!("error converting decrypted text to utf8 string: {}", e))?;
        // tracing::debug!("decrypted_text: {:?}", decrypted_text);
        Ok(decrypted_text)
    }

    pub fn generate_nonce_str() -> String {
        (0..32)
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
            .collect::<String>()
    }
}

fn xml_to_map(payload: &str) -> Result<HashMap<String, String>, WeixinError> {
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
            Err(e) => return Err(WeixinError::ApiError(format!("error parsing xml {}", e))),
            _ => {}
        }
    }
    Ok(m)
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

#[derive(Deserialize, Serialize, Debug)]
pub struct WxJSAPIResponse {
    pub return_code: String,
    pub return_msg: String,

    pub appid: Option<String>,
    pub mch_id: Option<String>,
    pub nonce_str: Option<String>,
    pub sign: Option<String>,
    pub result_code: Option<String>,
    pub err_code: Option<String>,
    pub err_code_des: Option<String>,

    pub trade_type: Option<String>,
    pub prepay_id: Option<String>,
}

impl V2ApiRequestPayload {
    pub fn new(
        charge_id: &str,         //
        wx_pub_app_id: &str,     // 微信公众号 app id
        wx_pub_mch_id: &str,     // 微信支付商户 id
        open_id: &str,           // 支付成功跳转
        client_ip: &str,         // 客户端 IP
        merchant_order_no: &str, // 商户订单号
        charge_amount: i32,      // 支付金额, 精确到分
        time_expire: i32,        // 过期时间 timestamp 精确到秒
        _subject: &str,          // 标题
        body: &str,              // 详情
    ) -> Result<Self, WeixinError> {
        let time_expire = chrono::DateTime::<chrono::Utc>::from_timestamp(time_expire as i64, 0)
            .ok_or_else(|| {
                WeixinError::MalformedRequest("can't convert timestamp to datetime".into())
            })?
            .with_timezone(&chrono::FixedOffset::east_opt(8 * 3600).unwrap())
            .format("%Y%m%d%H%M%S")
            .to_string();
        let total_fee = format!("{}", charge_amount);
        // create 32 charactors nonce string
        let nonce_str = v2api_md5::generate_nonce_str();
        let truncated_body = crate::utils::truncate_utf8(body, 127);
        let payload = V2ApiRequestPayload {
            appid: wx_pub_app_id.to_string(),
            mch_id: wx_pub_mch_id.to_string(),
            nonce_str,
            sign: String::from(""),
            // sign_type: "MD5",
            body: truncated_body.to_string(),
            out_trade_no: merchant_order_no.to_string(),
            total_fee,
            spbill_create_ip: client_ip.to_string(),
            time_expire,
            notify_url: crate::utils::charge_notify_url(charge_id),
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
        let mut m: HashMap<String, String> = serde_json::from_value(v.to_owned()).unwrap();
        m.remove("sign");
        let signature = v2api_md5::sign(&m, sign_key);
        self.sign = signature.clone();
        Ok(signature)
    }

    /**
     * https://pay.weixin.qq.com/wiki/doc/api/jsapi.php?chapter=7_7&index=6
     * https://pay.weixin.qq.com/wiki/doc/api/jsapi.php?chapter=9_1
     */
    pub async fn create_prepay_order(&self) -> Result<WxJSAPIResponse, WeixinError> {
        let xml_payload = quick_xml::se::to_string_with_root("xml", &self)
            .map_err(|e| WeixinError::Unexpected(format!("malformed xml payload: {}", e)))?;

        let res = reqwest::Client::new()
            .post("https://api.mch.weixin.qq.com/pay/unifiedorder")
            .body(xml_payload)
            .send()
            .await
            .map_err(|e| WeixinError::ApiError(format!("error request unifiedorder api: {}", e)))?;
        let res_text = res.text().await.map_err(|e| {
            WeixinError::ApiError(format!("error read unifiedorder response: {}", e))
        })?;
        tracing::debug!("unifiedorder response: {:?}", res_text);

        let res_obj: WxJSAPIResponse = quick_xml::de::from_str(&res_text).map_err(|e| {
            WeixinError::ApiError(format!("error deserialize WxJSAPIResponse: {}", e))
        })?;
        if res_obj.return_code != "SUCCESS" {
            return Err(WeixinError::ApiError(format!(
                "unifiedorder return_code != SUCCESS: {}",
                &res_obj.return_msg
            )));
        }
        if res_obj.result_code != Some("SUCCESS".to_string()) {
            return Err(WeixinError::ApiError(format!(
                "unifiedorder result_code != SUCCESS: {:?}",
                res_obj.err_code_des.as_ref()
            )));
        }

        Ok(res_obj)
    }
}

pub struct V2ApiNotifyPayload {
    pub result_code: String,
    pub merchant_order_no: String,
    pub amount: i32,
    signature: String,
    m: HashMap<String, String>,
}

impl V2ApiNotifyPayload {
    pub fn new(payload: &str) -> Result<Self, WeixinError> {
        let m = xml_to_map(payload)?;

        if m.get("return_code") != Some(&"SUCCESS".to_string()) {
            return Err(WeixinError::ApiError("return_code not SUCCESS".into()));
        }

        fn missing_params() -> WeixinError {
            WeixinError::ApiError("missing required params".into())
        }

        let signature = m.get("sign").ok_or_else(missing_params)?;
        let result_code = m.get("result_code").ok_or_else(missing_params)?;
        let out_trade_no = m.get("out_trade_no").ok_or_else(missing_params)?;
        let total_fee = m.get("total_fee").ok_or_else(missing_params)?;

        let amount = (total_fee
            .parse::<f64>()
            .map_err(|_| WeixinError::ApiError("invalid total_fee".into()))?
            * 100.0) as i32;

        Ok(Self {
            result_code: result_code.to_owned(),
            merchant_order_no: out_trade_no.to_owned(),
            amount,
            signature: signature.to_owned(),
            m,
        })
    }

    pub fn verify_md5_sign(&self, sign_key: &str) -> Result<(), WeixinError> {
        let mut m = self.m.clone();
        // k != "sign";
        m.remove("sign");
        let verified = v2api_md5::verify(&m, &self.signature, sign_key);
        if !verified {
            return Err(WeixinError::ApiError("wrong md5 signature".into()));
        }
        Ok(())
    }
}

#[derive(Debug, Serialize)]
pub struct V2ApiRefundPayload {
    pub appid: String,
    pub mch_id: String,
    pub nonce_str: String,
    pub sign: String,
    // pub sign_type: String,
    pub out_trade_no: String,
    pub out_refund_no: String,
    pub total_fee: String,
    pub refund_fee: String,
    pub notify_url: String,
}

impl V2ApiRefundPayload {
    pub fn new(
        refund_id: &str,
        charge_id: &str, //
        wx_pub_app_id: &str,
        wx_pub_mch_id: &str,
        charge_merchant_order_no: &str,
        refund_merchant_order_no: &str,
        charge_amount: i32,
        refund_amount: i32,
        _description: &str,
    ) -> Result<Self, WeixinError> {
        let nonce_str = v2api_md5::generate_nonce_str();
        Ok(Self {
            appid: wx_pub_app_id.to_string(),
            mch_id: wx_pub_mch_id.to_string(),
            nonce_str,
            sign: String::from(""),
            // sign_type: "MD5",
            out_trade_no: charge_merchant_order_no.to_string(),
            out_refund_no: refund_merchant_order_no.to_string(),
            total_fee: charge_amount.to_string(),
            refund_fee: refund_amount.to_string(),
            notify_url: crate::utils::refund_notify_url(charge_id, refund_id),
        })
    }

    pub fn sign_md5(&mut self, sign_key: &str) -> Result<String, WeixinError> {
        // 这里 deserialize 不会出问题
        let v = serde_json::to_value(&self).unwrap();
        let mut m: HashMap<String, String> = serde_json::from_value(v.to_owned()).unwrap();
        m.remove("sign");
        let signature = v2api_md5::sign(&m, sign_key);
        self.sign = signature.clone();
        Ok(signature)
    }

    /**
     * https://pay.weixin.qq.com/wiki/doc/api/jsapi.php?chapter=9_4
     */
    pub async fn send_request(
        &self,
        client_cert: &str,
        client_key: &str,
    ) -> Result<serde_json::Value, WeixinError> {
        let xml_payload = quick_xml::se::to_string_with_root("xml", &self)
            .map_err(|e| WeixinError::Unexpected(format!("malformed xml payload: {}", e)))?;

        let client = {
            // let cert = reqwest::Certificate::from_pem(client_cert.as_bytes())
            //     .map_err(|e| WeixinError::InvalidConfig(format!("error parsing client_cert: {}", e)))?;
            // let key = reqwest::Certificate::from_pem(client_key.as_bytes())
            //     .map_err(|e| WeixinError::InvalidConfig(format!("error parsing client_key: {}", e)))?;
            let identity =
                reqwest::Identity::from_pkcs8_pem(client_cert.as_bytes(), client_key.as_bytes())
                    .map_err(|e| {
                        WeixinError::InvalidConfig(format!("error creating identity: {}", e))
                    })?;
            reqwest::Client::builder()
                .identity(identity)
                .build()
                .map_err(|e| {
                    WeixinError::Unexpected(format!("error building reqwest client: {}", e))
                })?
        };

        let res = client // reqwest::Client::new()
            .post("https://api.mch.weixin.qq.com/secapi/pay/refund")
            .body(xml_payload)
            .send()
            .await
            .map_err(|e| WeixinError::ApiError(format!("error request wx refund api: {}", e)))?;
        let res_text = res.text().await.map_err(|e| {
            WeixinError::ApiError(format!("error read wx refund api response: {}", e))
        })?;
        tracing::debug!("wx refund api response: {:?}", res_text);

        let m = xml_to_map(&res_text)?;
        let res_obj = serde_json::to_value(&m).map_err(|e| {
            WeixinError::ApiError(format!("error serializing wx refund api response: {:?}", e))
        })?;

        if res_obj["return_code"].as_str() != Some("SUCCESS") {
            return Err(WeixinError::ApiError(format!(
                "wx refund api return_code != SUCCESS: {}",
                res_obj["return_msg"].as_str().unwrap_or_default()
            )));
        }

        Ok(res_obj)
    }
}

pub struct V2ApiRefundNotifyPayload {
    pub refund_status: String,
    pub merchant_order_no: String, // 商户订单号
    pub refund_id: String,         // pingxx-proxy-server 系统里 refund 的 id
    pub amount: i32,               // 退款金额
}

impl V2ApiRefundNotifyPayload {
    pub fn new(payload: &str, sign_key: &str) -> Result<Self, WeixinError> {
        let m = xml_to_map(payload)?;

        if m.get("return_code") != Some(&"SUCCESS".to_string()) {
            return Err(WeixinError::ApiError("return_code not SUCCESS".into()));
        }

        fn missing_params() -> WeixinError {
            WeixinError::ApiError("missing required params".into())
        }

        let req_info_encrypted = m.get("req_info").ok_or_else(missing_params)?;
        let req_info_xml = v2api_md5::decrypt_aes256_ecb(req_info_encrypted, sign_key)
            .map_err(|e| WeixinError::ApiError(format!("error decrypting req_info: {}", e)))?;
        let m = xml_to_map(&req_info_xml)?;
        tracing::debug!("req_info_xml: {:?}", m);

        // {"success_time": "2024-06-23 21:38:08", "refund_status": "SUCCESS", "refund_request_source": "API", "settlement_refund_fee": "10", "settlement_total_fee": "10", "total_fee": "10", "out_refund_no": "re_171914988503160517598425", "cash_refund_fee": "10", "refund_recv_accout": "支付用户零钱", "out_trade_no": "79320240623213641748", "transaction_id": "4200002184202406235936022560", "refund_account": "REFUND_SOURCE_RECHARGE_FUNDS", "refund_id": "50303410082024062384138175860", "refund_fee": "10"}

        let refund_status = m.get("refund_status").ok_or_else(missing_params)?;
        let out_trade_no = m.get("out_trade_no").ok_or_else(missing_params)?;
        let out_refund_no = m.get("out_refund_no").ok_or_else(missing_params)?;
        // let total_fee = m.get("total_fee").ok_or_else(missing_params)?;
        let refund_fee = m.get("refund_fee").ok_or_else(missing_params)?;

        let amount = (refund_fee
            .parse::<f64>()
            .map_err(|_| WeixinError::ApiError("invalid refund_fee".into()))?
            * 100.0) as i32;

        Ok(Self {
            refund_status: refund_status.to_owned(),
            merchant_order_no: out_trade_no.to_owned(),
            refund_id: out_refund_no.to_owned(),
            amount,
        })
    }
}

// ============================================================
// V3 API (RSA-SHA256 + JSON) — 新增
// ============================================================

pub mod v3api_rsa {
    use super::*;
    use openssl::hash::MessageDigest;
    use openssl::pkey::PKey;
    use openssl::sign::Signer;

    /// 用商户 RSA 私钥对签名串进行 SHA256WithRSA 签名，返回 Base64 编码结果
    pub fn sign_sha256_rsa(message: &str, private_key_pem: &str) -> Result<String, WeixinError> {
        let pkey = PKey::private_key_from_pem(private_key_pem.as_bytes()).map_err(|e| {
            WeixinError::InvalidConfig(format!("invalid merchant private key: {}", e))
        })?;
        let mut signer = Signer::new(MessageDigest::sha256(), &pkey)
            .map_err(|e| WeixinError::Unexpected(format!("error creating signer: {}", e)))?;
        signer
            .update(message.as_bytes())
            .map_err(|e| WeixinError::Unexpected(format!("error signing: {}", e)))?;
        let signature = signer
            .sign_to_vec()
            .map_err(|e| WeixinError::Unexpected(format!("error finalizing signature: {}", e)))?;
        Ok(data_encoding::BASE64.encode(&signature))
    }

    /// 构造 V3 API 的 Authorization 请求头
    /// https://pay.weixin.qq.com/wiki/doc/apiv3/wechatpay/wechatpay4_0.shtml
    pub fn build_authorization(
        method: &str,
        url_path: &str,
        body: &str,
        mch_id: &str,
        serial_no: &str,
        private_key_pem: &str,
    ) -> Result<String, WeixinError> {
        let timestamp = chrono::Utc::now().timestamp().to_string();
        let nonce_str = v2api_md5::generate_nonce_str();
        let sign_message = format!(
            "{}\n{}\n{}\n{}\n{}\n",
            method, url_path, timestamp, nonce_str, body
        );
        let signature = sign_sha256_rsa(&sign_message, private_key_pem)?;
        Ok(format!(
            "WECHATPAY2-SHA256-RSA2048 mchid=\"{}\",nonce_str=\"{}\",timestamp=\"{}\",serial_no=\"{}\",signature=\"{}\"",
            mch_id, nonce_str, timestamp, serial_no, signature
        ))
    }

    /// 用微信支付公钥验证应答/回调签名
    pub fn verify_sha256_rsa(
        message: &str,
        signature_b64: &str,
        public_key_pem: &str,
    ) -> Result<bool, WeixinError> {
        let pkey = PKey::public_key_from_pem(public_key_pem.as_bytes())
            .map_err(|e| WeixinError::InvalidConfig(format!("invalid wechat public key: {}", e)))?;
        let signature_bytes = data_encoding::BASE64
            .decode(signature_b64.as_bytes())
            .map_err(|e| WeixinError::ApiError(format!("invalid base64 signature: {}", e)))?;
        let mut verifier = openssl::sign::Verifier::new(MessageDigest::sha256(), &pkey)
            .map_err(|e| WeixinError::Unexpected(format!("error creating verifier: {}", e)))?;
        verifier
            .update(message.as_bytes())
            .map_err(|e| WeixinError::Unexpected(format!("error verifying: {}", e)))?;
        let result = verifier
            .verify(&signature_bytes)
            .map_err(|e| WeixinError::Unexpected(format!("error finalizing verify: {}", e)))?;
        Ok(result)
    }

    /// AEAD_AES_256_GCM 解密（V3 回调通知解密）
    /// https://pay.weixin.qq.com/wiki/doc/apiv3/wechatpay/wechatpay4_2.shtml
    pub fn decrypt_aes256_gcm(
        ciphertext_b64: &str,
        nonce: &str,
        associated_data: &str,
        apiv3_key: &str,
    ) -> Result<String, WeixinError> {
        let ciphertext = data_encoding::BASE64
            .decode(ciphertext_b64.as_bytes())
            .map_err(|e| WeixinError::ApiError(format!("invalid base64 ciphertext: {}", e)))?;

        // AES-256-GCM: 密文最后 16 字节是 tag
        if ciphertext.len() < 16 {
            return Err(WeixinError::ApiError("ciphertext too short".into()));
        }
        let (encrypted, tag) = ciphertext.split_at(ciphertext.len() - 16);

        let cipher = openssl::symm::Cipher::aes_256_gcm();
        let decrypted = openssl::symm::decrypt_aead(
            cipher,
            apiv3_key.as_bytes(),
            Some(nonce.as_bytes()),
            associated_data.as_bytes(),
            encrypted,
            tag,
        )
        .map_err(|e| WeixinError::ApiError(format!("error decrypting aes-256-gcm: {}", e)))?;

        String::from_utf8(decrypted)
            .map_err(|e| WeixinError::ApiError(format!("decrypted text is not valid utf8: {}", e)))
    }

    pub fn generate_nonce_str() -> String {
        v2api_md5::generate_nonce_str()
    }
}

// ============================================================
// V3 JSAPI 下单
// ============================================================

#[derive(Debug, Serialize)]
pub struct V3JsapiRequestPayload {
    pub appid: String,
    pub mchid: String,
    pub description: String,
    pub out_trade_no: String,
    pub time_expire: String, // RFC3339
    pub notify_url: String,
    pub amount: V3Amount,
    pub payer: V3Payer,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub scene_info: Option<V3SceneInfo>,
}

#[derive(Debug, Serialize)]
pub struct V3Amount {
    pub total: i32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub currency: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct V3Payer {
    pub openid: String,
}

#[derive(Debug, Serialize)]
pub struct V3SceneInfo {
    pub payer_client_ip: String,
}

#[derive(Debug, Deserialize)]
pub struct V3PrepayResponse {
    pub prepay_id: String,
}

impl V3JsapiRequestPayload {
    pub fn new(
        charge_id: &str,
        app_id: &str,
        mch_id: &str,
        open_id: &str,
        client_ip: &str,
        merchant_order_no: &str,
        charge_amount: i32,
        time_expire: i32,
        _subject: &str,
        body: &str,
    ) -> Result<Self, WeixinError> {
        let time_expire_rfc3339 =
            chrono::DateTime::<chrono::Utc>::from_timestamp(time_expire as i64, 0)
                .ok_or_else(|| {
                    WeixinError::MalformedRequest("can't convert timestamp to datetime".into())
                })?
                .with_timezone(&chrono::FixedOffset::east_opt(8 * 3600).unwrap())
                .to_rfc3339();

        let truncated_body = crate::utils::truncate_utf8(body, 127);

        Ok(Self {
            appid: app_id.to_string(),
            mchid: mch_id.to_string(),
            description: truncated_body.to_string(),
            out_trade_no: merchant_order_no.to_string(),
            time_expire: time_expire_rfc3339,
            notify_url: crate::utils::charge_notify_url(charge_id),
            amount: V3Amount {
                total: charge_amount,
                currency: None,
            },
            payer: V3Payer {
                openid: open_id.to_string(),
            },
            scene_info: Some(V3SceneInfo {
                payer_client_ip: client_ip.to_string(),
            }),
        })
    }

    /// 调用 V3 JSAPI 下单接口
    pub async fn create_prepay_order(
        &self,
        mch_id: &str,
        serial_no: &str,
        private_key_pem: &str,
    ) -> Result<V3PrepayResponse, WeixinError> {
        let url_path = "/v3/pay/transactions/jsapi";
        let body = serde_json::to_string(&self)
            .map_err(|e| WeixinError::Unexpected(format!("error serializing payload: {}", e)))?;

        let authorization = v3api_rsa::build_authorization(
            "POST",
            url_path,
            &body,
            mch_id,
            serial_no,
            private_key_pem,
        )?;

        let url = format!("https://api.mch.weixin.qq.com{}", url_path);
        let res = reqwest::Client::new()
            .post(&url)
            .header("Authorization", &authorization)
            .header("Accept", "application/json")
            .header("Content-Type", "application/json")
            .header("User-Agent", "pingxx-proxy-server")
            .body(body)
            .send()
            .await
            .map_err(|e| WeixinError::ApiError(format!("error request v3 jsapi: {}", e)))?;

        let status = res.status();
        let res_text = res
            .text()
            .await
            .map_err(|e| WeixinError::ApiError(format!("error read v3 jsapi response: {}", e)))?;
        tracing::debug!("v3 jsapi response [{}]: {:?}", status, res_text);

        if !status.is_success() {
            return Err(WeixinError::ApiError(format!(
                "v3 jsapi returned {}: {}",
                status, res_text
            )));
        }

        let res_obj: V3PrepayResponse = serde_json::from_str(&res_text).map_err(|e| {
            WeixinError::ApiError(format!("error deserializing v3 prepay response: {}", e))
        })?;

        Ok(res_obj)
    }
}

// ============================================================
// V3 支付回调通知
// ============================================================

/// V3 回调通知外层结构（通用）
#[derive(Debug, Deserialize)]
pub struct V3NotifyPayload {
    pub id: String,
    pub event_type: String,
    pub resource: V3NotifyResource,
}

#[derive(Debug, Deserialize)]
pub struct V3NotifyResource {
    pub algorithm: String,
    pub ciphertext: String,
    pub nonce: String,
    #[serde(default)]
    pub associated_data: String,
}

/// V3 支付成功回调解密后的数据
#[derive(Debug, Deserialize)]
pub struct V3ChargeNotifyDecrypted {
    pub trade_state: String,    // SUCCESS, REFUND, NOTPAY, CLOSED, etc.
    pub out_trade_no: String,   // 商户订单号
    pub transaction_id: String, // 微信支付订单号
    pub amount: V3NotifyAmount,
}

#[derive(Debug, Deserialize)]
pub struct V3NotifyAmount {
    pub total: i32,
    pub payer_total: Option<i32>,
}

impl V3NotifyPayload {
    /// 从 JSON body 解析 V3 回调通知
    pub fn from_json(payload: &str) -> Result<Self, WeixinError> {
        serde_json::from_str(payload)
            .map_err(|e| WeixinError::ApiError(format!("error parsing v3 notify json: {}", e)))
    }

    /// 解密 resource 得到支付结果
    pub fn decrypt_charge_notify(
        &self,
        apiv3_key: &str,
    ) -> Result<V3ChargeNotifyDecrypted, WeixinError> {
        let plaintext = v3api_rsa::decrypt_aes256_gcm(
            &self.resource.ciphertext,
            &self.resource.nonce,
            &self.resource.associated_data,
            apiv3_key,
        )?;
        tracing::debug!("v3 charge notify decrypted: {:?}", plaintext);
        serde_json::from_str(&plaintext).map_err(|e| {
            WeixinError::ApiError(format!("error deserializing v3 charge notify: {}", e))
        })
    }

    /// 解密 resource 得到退款结果
    pub fn decrypt_refund_notify(
        &self,
        apiv3_key: &str,
    ) -> Result<V3RefundNotifyDecrypted, WeixinError> {
        let plaintext = v3api_rsa::decrypt_aes256_gcm(
            &self.resource.ciphertext,
            &self.resource.nonce,
            &self.resource.associated_data,
            apiv3_key,
        )?;
        tracing::debug!("v3 refund notify decrypted: {:?}", plaintext);
        serde_json::from_str(&plaintext).map_err(|e| {
            WeixinError::ApiError(format!("error deserializing v3 refund notify: {}", e))
        })
    }
}

// ============================================================
// V3 退款
// ============================================================

#[derive(Debug, Serialize)]
pub struct V3RefundRequestPayload {
    pub out_trade_no: String,
    pub out_refund_no: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
    pub notify_url: String,
    pub amount: V3RefundAmount,
}

#[derive(Debug, Serialize)]
pub struct V3RefundAmount {
    pub refund: i32,
    pub total: i32,
    pub currency: String,
}

#[derive(Debug, Deserialize)]
pub struct V3RefundResponse {
    pub refund_id: Option<String>,
    pub out_refund_no: Option<String>,
    pub status: Option<String>, // SUCCESS, CLOSED, PROCESSING, ABNORMAL
    #[serde(flatten)]
    pub extra: serde_json::Value,
}

/// V3 退款回调解密后的数据
#[derive(Debug, Deserialize)]
pub struct V3RefundNotifyDecrypted {
    pub refund_status: String, // SUCCESS, CLOSED, ABNORMAL
    pub out_trade_no: String,  // 商户订单号
    pub out_refund_no: String, // 商户退款单号
    pub refund_id: String,     // 微信退款单号
    pub amount: V3RefundNotifyAmount,
}

#[derive(Debug, Deserialize)]
pub struct V3RefundNotifyAmount {
    pub total: i32,
    pub refund: i32,
    pub payer_total: Option<i32>,
    pub payer_refund: Option<i32>,
}

impl V3RefundRequestPayload {
    pub fn new(
        refund_id: &str,
        charge_id: &str,
        charge_merchant_order_no: &str,
        refund_merchant_order_no: &str,
        charge_amount: i32,
        refund_amount: i32,
        description: &str,
    ) -> Self {
        Self {
            out_trade_no: charge_merchant_order_no.to_string(),
            out_refund_no: refund_merchant_order_no.to_string(),
            reason: if description.is_empty() {
                None
            } else {
                Some(description.to_string())
            },
            notify_url: crate::utils::refund_notify_url(charge_id, refund_id),
            amount: V3RefundAmount {
                refund: refund_amount,
                total: charge_amount,
                currency: "CNY".to_string(),
            },
        }
    }

    /// 调用 V3 退款接口
    pub async fn send_request(
        &self,
        mch_id: &str,
        serial_no: &str,
        private_key_pem: &str,
    ) -> Result<V3RefundResponse, WeixinError> {
        let url_path = "/v3/refund/domestic/refunds";
        let body = serde_json::to_string(&self).map_err(|e| {
            WeixinError::Unexpected(format!("error serializing refund payload: {}", e))
        })?;

        let authorization = v3api_rsa::build_authorization(
            "POST",
            url_path,
            &body,
            mch_id,
            serial_no,
            private_key_pem,
        )?;

        let url = format!("https://api.mch.weixin.qq.com{}", url_path);
        let res = reqwest::Client::new()
            .post(&url)
            .header("Authorization", &authorization)
            .header("Accept", "application/json")
            .header("Content-Type", "application/json")
            .header("User-Agent", "pingxx-proxy-server")
            .body(body)
            .send()
            .await
            .map_err(|e| WeixinError::ApiError(format!("error request v3 refund api: {}", e)))?;

        let status = res.status();
        let res_text = res
            .text()
            .await
            .map_err(|e| WeixinError::ApiError(format!("error read v3 refund response: {}", e)))?;
        tracing::debug!("v3 refund response [{}]: {:?}", status, res_text);

        if !status.is_success() {
            return Err(WeixinError::ApiError(format!(
                "v3 refund api returned {}: {}",
                status, res_text
            )));
        }

        let res_obj: V3RefundResponse = serde_json::from_str(&res_text).map_err(|e| {
            WeixinError::ApiError(format!("error deserializing v3 refund response: {}", e))
        })?;

        Ok(res_obj)
    }
}
