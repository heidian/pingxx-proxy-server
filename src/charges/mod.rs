use axum::{
    http::StatusCode,
    response::Json,
    routing::{get, post},
    Router,
};
use serde::{Deserialize, Serialize};
use serde_json::json;

async fn test() -> &'static str {
    "ok"
}

#[derive(Deserialize, Debug)]
pub enum PaymentChannel {
    #[serde(rename = "alipay_pc_direct")]
    AlipayPcDirect,
    #[serde(rename = "alipay_wap")]
    AlipayWap,
    #[serde(rename = "wx_pub")]
    WxPub,
}

#[derive(Deserialize, Debug)]
pub struct ChargeExtra {
    pub success_url: Option<String>,
    pub cancel_url: Option<String>,
}

#[derive(Deserialize, Debug)]
pub struct PingxxApp {
    pub id: String,
}

#[derive(Deserialize, Debug)]
pub struct CreateChargeRequestPayload {
    pub order_no: String,
    pub amount: u32,
    pub channel: PaymentChannel,
    pub client_ip: String,
    pub subject: String,
    pub body: String,
    pub currency: String,
    pub time_expire: u32,
    pub extra: ChargeExtra,
    pub app: PingxxApp,
}

/// 实现 Requester 接口的各种方法
#[derive(Debug, Default, Serialize, Deserialize)]
pub struct AlipayPcDirectRequest {
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

async fn create_charge(body: String) -> Result<Json<serde_json::Value>, StatusCode> {
    // tracing::info!("create_charge: {}", body);
    let req_payload: CreateChargeRequestPayload = serde_json::from_str(&body).map_err(|e| {
        tracing::error!("create_charge: {}", e);
        StatusCode::BAD_REQUEST
    })?;
    tracing::info!("create_charge: {:?}", req_payload);

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

    let charge = json!({
        "id": "ch_101240601691280343040013",
        "object": "charge",
        "created": 1717238707,
        "livemode": true,
        "paid": false,
        "refunded": false,
        "reversed": false,
        "app": "app_qnnT0KyzvbfDaT0e",
        "channel": "alipay_pc_direct",
        "order_no": "98520240601184136264",
        "client_ip": "192.168.65.1",
        "amount": 800,
        "amount_settle": 800,
        "currency": "cny",
        "subject": "鬼骨孖的店铺",
        "body": "宝蓝色绑带高跟凉鞋",
        "extra": {
          "success_url": "https://dxd1234.heidianer.com/order/be4570fbd7bf99d17f3b68589a5a46c2a7b302c8?payment_status=success"
        },
        "time_paid": null,
        "time_expire": 1717240296,
        "time_settle": null,
        "transaction_no": null,
        "refunds": {
          "object": "list",
          "url": "/v1/charges/ch_101240601691280343040013/refunds",
          "has_more": false,
          "data": []
        },
        "amount_refunded": 0,
        "failure_code": null,
        "failure_msg": null,
        "metadata": {},
        "credential": {
          "object": "credential",
          "alipay_pc_direct": alipay_pc_direct_req,
        //   "alipay_pc_direct": {
        //     "service": "create_direct_pay_by_user",
        //     "_input_charset": "utf-8",
        //     "return_url": "https://dxd1234.heidianer.com/order/be4570fbd7bf99d17f3b68589a5a46c2a7b302c8?payment_status=success",
        //     "notify_url": "https://notify.pingxx.com/notify/charges/ch_101240601691280343040013",
        //     "partner": "2088612364840749",
        //     "out_trade_no": "98520240601184136264",
        //     "subject": "鬼骨孖的店铺",
        //     "body": "宝蓝色绑带高跟凉鞋",
        //     "total_fee": "8.00",
        //     "payment_type": 1,
        //     "seller_id": "2088612364840749",
        //     "it_b_pay": "26m",
        //     "sign": "LVbTntAiRJya33f5Z+ycjnT+6uOAl5vFlSTnrG63zR9UVXKOKe6U/79F077KmrMwmouXPN4SQ7FxOQE/al0jD02m0pZ+u4NCqQQvfG6QgJge+T5KlgEY7hHjhzttmra4XRqDnckqYSMcK4JXCjYaiCFK126jODllclUihvFzKuQ=",
        //     "sign_type": "RSA"
        //   }
        },
        "description": null
    });
    Ok(Json(charge))
}

pub fn get_routes() -> Router {
    Router::new()
        .route("/test", get(test))
        .route("/charges", post(create_charge))
}
