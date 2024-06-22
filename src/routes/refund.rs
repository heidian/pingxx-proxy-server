use crate::core::RefundError;
use serde::Deserialize;
use serde_json::json;

#[derive(Deserialize, Debug)]
pub struct CreateRefundRequestPayload {
    #[serde(rename = "charge")]
    pub charge_id: String,
    pub charge_amount: i32,
    pub description: String,
    pub funding_source: Option<String>, // 微信退款专用 unsettled_funds | recharge_funds
}

pub async fn create_refund(
    prisma_client: &crate::prisma::PrismaClient,
    order_id: String,
    refund_req_payload: CreateRefundRequestPayload,
) -> Result<serde_json::Value, RefundError> {
    tracing::info!("{:?}", refund_req_payload);
    let charge_id = &refund_req_payload.charge_id.clone();
    let (charge, order, app, sub_app) =
        crate::utils::load_charge_from_db(&prisma_client, &charge_id).await?;
    // let (order, app, sub_app) = crate::utils::load_order_from_db(&prisma_client, &order_id).await?;
    if order.id != order_id {
        return Err(RefundError::BadRequest(format!("charge {} doesn't belong to order {}", charge_id, order_id)));
    }
    let _ = charge;
    let _ = app;
    let _ = sub_app;
    Ok(json!({}))
}
