use serde::{Deserialize, Serialize};
use axum::{
    http::StatusCode,
    response::Json,
    routing::post,
    Router,
};
use rand::Rng;
use serde_json::json;


#[derive(Deserialize, Serialize, Debug)]
pub struct CreateOrderRequestPayload {
    pub app: String,               // ping++ 的商户系统的 appid
    pub receipt_app: String,       // 上面 appid 对应 app 里的子商户 id
    pub service_app: String,       // 上面 appid 对应 app 里的子商户 id
    pub uid: String,               // 业务系统里的用户 id
    pub merchant_order_no: String, // 业务系统里的交易 id
    pub amount: u32,
    pub client_ip: String,
    pub subject: String,
    pub body: String,
    pub currency: String,
    pub time_expire: u32,
}

#[derive(Deserialize, Serialize, Debug)]
pub struct Order {
    pub id: String,
    pub object: String,

    pub app: String,
    pub receipt_app: String,
    pub service_app: String,
    pub uid: String,
    pub merchant_order_no: String,
    pub amount: u32,
    pub client_ip: String,
    pub subject: String,
    pub body: String,
    pub currency: String,
    pub time_expire: u32,
}

async fn create_order(body: String) -> Result<Json<serde_json::Value>, StatusCode> {
    tracing::info!("create_order: {}", body);
    let req_payload: CreateOrderRequestPayload = serde_json::from_str(&body).map_err(|e| {
        tracing::error!("error parsing create_order request payload: {:?}", e);
        StatusCode::BAD_REQUEST
    })?;
    let timestamp = chrono::Utc::now().timestamp_millis();
    let order_id = {
        let mut rng = rand::thread_rng();
        let number: u64 = rng.gen_range(10000000000..100000000000);
        format!("o_{}{}", timestamp, number)
    };

    let mut order = json!({
        "id": order_id,
        "object": "order",
        "created": timestamp,
        "livemode": false,
        "paid": false,
        "refunded": false,
        "status": "created",
        "metadata": {},
        "charge_essentials": {},
    });

    order.as_object_mut().unwrap().extend({
        let req_payload_obj = serde_json::to_value(&req_payload).map_err(|e| {
            tracing::error!("error serializing create_charge response: {:?}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;
        req_payload_obj.as_object().unwrap().clone()
    });

    Ok(Json(order))
}

pub fn get_routes() -> Router {
    Router::new()
        .route("/orders", post(create_order))
}
