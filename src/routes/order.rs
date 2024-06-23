use super::charge::ChargeResponsePayload;
use crate::core::OrderError;
use serde::{Deserialize, Serialize};
use serde_json::json;

#[derive(Deserialize, Debug)]
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

#[derive(Serialize, Debug)]
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
    pub charges: serde_json::Value,
}

impl OrderResponsePayload {
    pub fn new(
        order: &crate::prisma::order::Data,
        charges: &Vec<crate::prisma::charge::Data>,
        app: &crate::prisma::app::Data,
        sub_app: &crate::prisma::sub_app::Data,
    ) -> Self {
        let charges = {
            // let empty: Vec<crate::prisma::charge::Data> = vec![];
            // let charges = order.charges.as_ref().unwrap_or(&empty);
            let data = charges
                .iter()
                .filter_map(|charge| {
                    match ChargeResponsePayload::new(charge) {
                        Ok(res) => res.to_json().ok(),
                        Err(_) => None
                    }
                })
                .collect::<Vec<serde_json::Value>>();
            json!({
                "object": "list",
                "url": "/v1/charges",
                "has_more": false,
                "data": data
            })
        };
        Self {
            id: order.id.clone(),
            object: String::from("order"),
            created: order.created_at.timestamp() as i32,
            app: app.id.clone(),
            receipt_app: sub_app.id.clone(),
            service_app: sub_app.id.clone(),
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
            charges,
        }
    }
}

pub async fn create_order(
    prisma_client: &crate::prisma::PrismaClient,
    req_payload: CreateOrderRequestPayload,
) -> Result<OrderResponsePayload, OrderError> {
    let order_id = crate::utils::generate_id("o_");

    prisma_client
        .order()
        .create(
            order_id.clone(),
            crate::prisma::app::id::equals(req_payload.app.clone()),
            crate::prisma::sub_app::id::equals(req_payload.service_app.clone()),
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
        .map_err(|e| OrderError::Unexpected(format!("sql error: {:?}", e)))?;

    let (order, charges, app, sub_app) = crate::utils::load_order_from_db(&prisma_client, &order_id).await?;
    let result = OrderResponsePayload::new(&order, &charges, &app, &sub_app);

    Ok(result)
}

pub async fn retrieve_order(
    prisma_client: &crate::prisma::PrismaClient,
    order_id: String,
) -> Result<serde_json::Value, OrderError> {
    let (order, charges, app, sub_app) = crate::utils::load_order_from_db(&prisma_client, &order_id).await?;
    let order_response = OrderResponsePayload::new(&order, &charges, &app, &sub_app);
    let mut result = serde_json::to_value(order_response).map_err(|e| {
        OrderError::Unexpected(format!("error serializing order response payload: {:?}", e))
    })?;
    let charge = order.charges.unwrap_or_default().first().cloned();
    if let Some(charge) = charge {
        result["charge_essentials"] = ChargeResponsePayload::new(&charge).map_err(|e| {
            OrderError::Unexpected(format!("{:?}", e))
        })?.to_json().map_err(|e| {
            OrderError::Unexpected(format!("error serializing charge essentials: {:?}", e))
        })?;
    }
    Ok(result)
}
