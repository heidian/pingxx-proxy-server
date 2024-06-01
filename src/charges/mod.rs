use axum::{
    http::StatusCode,
    routing::{get, post},
    response::Json,
    Router,
};
use serde::Deserialize;
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
pub struct  ChargeExtra {
    pub success_url: Option<String>,
    pub cancel_url: Option<String>,
}

#[derive(Deserialize, Debug)]
pub struct  PingxxApp {
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

async fn create_charge(body: String) -> Result<Json<serde_json::Value>, StatusCode> {
    // tracing::info!("create_charge: {}", body);
    let _req_payload: CreateChargeRequestPayload = serde_json::from_str(&body).map_err(|e| {
        tracing::error!("create_charge: {}", e);
        StatusCode::BAD_REQUEST
    })?;
    tracing::info!("create_charge: {:?}", _req_payload);
    let pingxx_params = std::env::var("PINGXX_PARAMS").unwrap();
    let pingxx_params: serde_json::Value = serde_json::from_str(&pingxx_params).unwrap();
    tracing::info!("PINGXX_PARAMS: {}", pingxx_params);
    let charge = json!({
        // 
    });
    Ok(Json(charge))
}

pub fn get_routes() -> Router {
    Router::new()
        .route("/test", get(test))
        .route("/charges", post(create_charge))
}
