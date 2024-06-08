use axum::{http::StatusCode, response::Json, routing::post, Router};
use rand::Rng;
use serde::{Deserialize, Serialize};
use serde_json::json;

#[derive(Deserialize, Serialize, Debug)]
pub struct CreateOrderRequestPayload {
    pub app: String,               // ping++ 的商户系统的 appid
    pub receipt_app: String,       // 上面 appid 对应 app 里的子商户 id
    pub service_app: String,       // 上面 appid 对应 app 里的子商户 id
    pub uid: String,               // 业务系统里的用户 id
    pub merchant_order_no: String, // 业务系统里的交易 id
    pub amount: i32,
    pub client_ip: String,
    pub subject: String,
    pub body: String,
    pub currency: String,
    pub time_expire: i32,
}

#[derive(Deserialize, Serialize, Debug)]
pub struct OrderResponsePayload {
    pub id: String,
    pub object: String,
    pub created: i32,
    pub app: String,
    pub receipt_app: String,
    pub service_app: String,
    pub uid: String,
    pub merchant_order_no: String,
    pub status: String,
    pub paid: bool,
    pub refunded: bool,
    pub amount: i32,
    pub amount_paid: i32,
    pub amount_refunded: i32,
    pub client_ip: String,
    pub subject: String,
    pub body: String,
    pub currency: String,
    pub time_paid: Option<i32>,
    pub time_expire: i32,
    pub metadata: serde_json::Value,
}

impl OrderResponsePayload {
    pub fn new(
        order: &crate::prisma::order::Data,
        app: &crate::prisma::app::Data,
        sub_app: &crate::prisma::sub_app::Data,
    ) -> Self {
        Self {
            id: order.order_id.clone(),
            object: String::from("order"),
            created: order.created_at.timestamp() as i32,
            app: app.key.clone(),
            receipt_app: sub_app.key.clone(),
            service_app: sub_app.key.clone(),
            uid: order.uid.clone(),
            merchant_order_no: order.merchant_order_no.clone(),
            status: order.status.clone(),
            paid: order.paid,
            refunded: order.refunded,
            amount: order.amount,
            amount_paid: order.amount_paid,
            amount_refunded: order.amount_refunded,
            client_ip: order.client_ip.clone(),
            subject: order.subject.clone(),
            body: order.body.clone(),
            currency: order.currency.clone(),
            time_paid: None,
            time_expire: order.time_expire,
            metadata: order.metadata.clone(),
        }
    }

}

async fn create_order(body: String) -> Result<Json<OrderResponsePayload>, StatusCode> {
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

    let prisma_client = crate::prisma::new_client().await.map_err(|e| {
        tracing::error!("error creating prisma client: {:?}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    let order = prisma_client
        .order()
        .create(
            crate::prisma::app::key::equals(req_payload.app.clone()),
            crate::prisma::sub_app::key::equals(req_payload.service_app.clone()),
            order_id,
            req_payload.uid,
            req_payload.merchant_order_no,
            String::from("created"),
            false,
            false,
            req_payload.amount,
            0,
            0,
            req_payload.client_ip,
            req_payload.subject,
            req_payload.body,
            req_payload.currency,
            req_payload.time_expire,
            json!({}),
            vec![],
        )
        .exec()
        .await
        .map_err(|e| {
            tracing::error!("error creating order: {:?}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    let result = OrderResponsePayload {
        id: order.order_id.clone(),
        object: String::from("order"),
        created: order.created_at.timestamp() as i32,
        app: req_payload.app,
        receipt_app: req_payload.receipt_app,
        service_app: req_payload.service_app,
        uid: order.uid,
        merchant_order_no: order.merchant_order_no,
        status: order.status,
        paid: order.paid,
        refunded: order.refunded,
        amount: order.amount,
        amount_paid: order.amount_paid,
        amount_refunded: order.amount_refunded,
        client_ip: order.client_ip,
        subject: order.subject,
        body: order.body,
        currency: order.currency,
        time_paid: None,
        time_expire: order.time_expire,
        metadata: order.metadata,
    };

    Ok(Json(result))
}

pub fn get_routes() -> Router {
    Router::new().route("/orders", post(create_order))
}
