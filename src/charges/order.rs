use axum::{extract::Path, http::StatusCode, response::Json};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::str::FromStr;

use super::charge::{ChargeResponsePayload, PaymentChannel};

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

pub async fn load_order_from_db(
    prisma_client: &crate::prisma::PrismaClient,
    order_id: &str,
) -> Result<
    (
        crate::prisma::order::Data,
        crate::prisma::app::Data,
        crate::prisma::sub_app::Data,
    ),
    StatusCode,
> {
    let order = prisma_client
        .order()
        .find_unique(crate::prisma::order::order_id::equals(order_id.to_string()))
        .with(crate::prisma::order::sub_app::fetch())
        .with(crate::prisma::order::app::fetch())
        .with(
            crate::prisma::order::charges::fetch(vec![
                // crate::prisma::charge::is_valid::equals(true)
            ])
            .order_by(crate::prisma::charge::created_at::order(
                prisma_client_rust::Direction::Desc,
            )), // .take(1),
        )
        .exec()
        .await
        .map_err(|e| {
            tracing::error!("sql error: {:?}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?
        .ok_or_else(|| {
            tracing::error!("order not found");
            StatusCode::NOT_FOUND
        })?;

    let (app, sub_app) = {
        let order = order.clone();
        let app = order.app.ok_or_else(|| {
            tracing::error!("order.app is None");
            StatusCode::INTERNAL_SERVER_ERROR
        })?;
        let sub_app = order.sub_app.ok_or_else(|| {
            tracing::error!("order.sub_app is None");
            StatusCode::INTERNAL_SERVER_ERROR
        })?;
        (*app, *sub_app)
    };

    Ok((order, app, sub_app))
}

pub async fn create_order(body: String) -> Result<Json<OrderResponsePayload>, StatusCode> {
    tracing::info!("create_order: {}", body);
    let req_payload: CreateOrderRequestPayload = serde_json::from_str(&body).map_err(|e| {
        tracing::error!("error parsing create_order request payload: {:?}", e);
        StatusCode::BAD_REQUEST
    })?;

    let order_id = crate::utils::generate_id("o_");

    let prisma_client = crate::prisma::new_client().await.map_err(|e| {
        tracing::error!("error creating prisma client: {:?}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    prisma_client
        .order()
        .create(
            crate::prisma::app::key::equals(req_payload.app.clone()),
            crate::prisma::sub_app::key::equals(req_payload.service_app.clone()),
            order_id.clone(),
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

    let (order, app, sub_app) = load_order_from_db(&prisma_client, &order_id).await?;
    let result = OrderResponsePayload::new(&order, &app, &sub_app);

    Ok(Json(result))
}

pub async fn retrieve_order(
    Path(order_id): Path<String>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let prisma_client = crate::prisma::new_client().await.map_err(|e| {
        tracing::error!("error creating prisma client: {:?}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;
    let (order, app, sub_app) = load_order_from_db(&prisma_client, &order_id).await?;
    let order_response = OrderResponsePayload::new(&order, &app, &sub_app);
    let mut result = serde_json::to_value(order_response).map_err(|e| {
        tracing::error!("error serializing order response payload: {:?}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;
    let charge = order.charges.unwrap_or_default().first().cloned();
    if let Some(charge) = charge {
        let channel = PaymentChannel::from_str(&charge.channel).map_err(|e| {
            tracing::error!("error parsing charge channel: {:?}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;
        let charge_response = ChargeResponsePayload {
            id: charge.charge_id,
            object: "charge".to_string(),
            is_valid: charge.is_valid,
            channel,
            amount: charge.amount,
            extra: charge.extra,
            credential: charge.credential,
        };
        result["charge_essentials"] = serde_json::to_value(charge_response).map_err(|e| {
            tracing::error!("error serializing charge essentials: {:?}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;
    }
    Ok(Json(result))
}
