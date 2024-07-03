use crate::core::{OrderError, OrderResponse};
use serde::Deserialize;
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

pub async fn create_order(
    prisma_client: &crate::prisma::PrismaClient,
    req_payload: CreateOrderRequestPayload,
) -> Result<OrderResponse, OrderError> {
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

    let (order, charges, app, sub_app) =
        crate::utils::load_order_from_db(&prisma_client, &order_id).await?;
    let order_response: OrderResponse = (&order, None, &charges, &app, &sub_app).into();
    Ok(order_response)
}

pub async fn retrieve_order(
    prisma_client: &crate::prisma::PrismaClient,
    order_id: String,
) -> Result<serde_json::Value, OrderError> {
    let (order, charges, app, sub_app) =
        crate::utils::load_order_from_db(&prisma_client, &order_id).await?;
    let first_charge = order
        .charges
        .as_ref()
        .cloned()
        .unwrap_or_default()
        .first()
        .cloned();

    let order_response: OrderResponse =
        (&order, first_charge.as_ref(), &charges, &app, &sub_app).into();
    let result = serde_json::to_value(order_response).map_err(|e| {
        OrderError::Unexpected(format!("error serializing order response payload: {:?}", e))
    })?;
    Ok(result)
}
