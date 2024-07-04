use crate::core::{OrderResponse, ChargeResponse};
use serde_json::json;

pub(super) async fn send_order_charge_webhook(
    app_webhook_url: &str,
    app: &crate::prisma::app::Data,
    sub_app: &crate::prisma::sub_app::Data,
    order: &crate::prisma::order::Data,
    charges: &Vec<crate::prisma::charge::Data>,
    charge: &crate::prisma::charge::Data,
) -> Result<(), ()> {
    let order_response: OrderResponse = (order, Some(charge), charges, app, sub_app).into();

    let event_data = serde_json::to_value(order_response).map_err(|e| {
        tracing::error!("error serializing order response payload: {:?}", e);
    })?;

    let event_payload = json!({
        "id": crate::utils::generate_id("evt_"),
        "object": "event",
        "created": chrono::Utc::now().timestamp(),
        "type": "order.succeeded",
        "data": {
            "object": event_data
        },
    });

    // let app_webhook_url = std::env::var("APP_WEBHOOK_URL").expect("APP_WEBHOOK_URL must be set");
    let res = reqwest::Client::new()
        .post(app_webhook_url)
        .json(&event_payload)
        .send()
        .await
        .map_err(|e| {
            tracing::error!("error sending webhook: {:?}", e);
        })?;
    tracing::info!("webhook response {} {:?}", res.status(), res.text().await);

    Ok(())
}

pub(super) async fn send_basic_charge_webhook(
    app_webhook_url: &str,
    app: &crate::prisma::app::Data,
    refunds: &Vec<crate::prisma::refund::Data>,
    charge: &crate::prisma::charge::Data,
) -> Result<(), ()> {
    let charge_response: ChargeResponse = (charge, refunds, app).into();
    let mut event_data = serde_json::to_value(charge_response).map_err(|e| {
        tracing::error!("error serializing charge response payload: {:?}", e);
    })?;
    event_data["order_no"] = event_data["merchant_order_no"].clone();

    let event_payload = json!({
        "id": crate::utils::generate_id("evt_"),
        "object": "event",
        "created": chrono::Utc::now().timestamp(),
        "type": "charge.succeeded",
        "data": {
            "object": event_data
        },
    });

    // let app_webhook_url = std::env::var("APP_WEBHOOK_URL").expect("APP_WEBHOOK_URL must be set");
    let res = reqwest::Client::new()
        .post(app_webhook_url)
        .json(&event_payload)
        .send()
        .await
        .map_err(|e| {
            tracing::error!("error sending webhook: {:?}", e);
        })?;
    tracing::info!("webhook response {} {:?}", res.status(), res.text().await);

    Ok(())
}


pub(super) async fn send_refund_webhook(
    _app: &crate::prisma::app::Data,
    _sub_app: &crate::prisma::sub_app::Data,
    _order: &crate::prisma::order::Data,
    _refund: &crate::prisma::refund::Data,
) -> Result<(), ()> {
    // 不发送 refund webhook, 现在 ping++ 发送的 order.refunded webhook 没有 refund 信息, 只能靠主动查询
    Ok(())
}
