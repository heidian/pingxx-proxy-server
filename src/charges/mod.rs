use axum::{
    extract::Path,
    http::StatusCode,
    response::Json,
    routing::{get, post},
    Router,
};
use rand::Rng;
use serde_json::json;

mod alipay;
mod charge;

use crate::orders::Order;
use charge::{CreateChargeRequestPayload, PaymentChannel};

async fn test() -> String {
    let charge_id = {
        let mut rng = rand::thread_rng();
        let timestamp = chrono::Utc::now().timestamp_millis();
        let number: u64 = rng.gen_range(10000000000..100000000000);
        format!("ch_{}{}", timestamp, number)
    };
    charge_id
}

async fn create_charge(
    Path(order_id): Path<String>,
    // body: String,
    Json(charge_req_payload): Json<CreateChargeRequestPayload>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    // tracing::info!("create_charge: {}", body);
    // let charge_req_payload: CreateChargeRequestPayload =
    //     serde_json::from_str(&body).map_err(|e| {
    //         tracing::error!("error parsing create_charge request payload: {:?}", e);
    //         StatusCode::BAD_REQUEST
    //     })?;
    let timestamp = chrono::Utc::now().timestamp_millis();
    let charge_id = {
        let mut rng = rand::thread_rng();
        let number: u64 = rng.gen_range(10000000000..100000000000);
        format!("ch_{}{}", timestamp, number)
    };

    let charge_notify_url_root = std::env::var("CHARGE_NOTIFY_URL_ROOT").unwrap();
    let notify_url = format!("{}{}", charge_notify_url_root, charge_id);
    // "https://notify.pingxx.com/notify/charges/ch_101240601691280343040013";

    let order = Order {
        id: order_id,
        object: "order".to_string(),
        app: "app_test".to_string(),
        receipt_app: "app_test".to_string(),
        service_app: "app_test".to_string(),
        uid: "user_test".to_string(),
        merchant_order_no: "TEST2001708140000017551".to_string(),
        amount: charge_req_payload.charge_amount,
        client_ip: "".to_string(),
        subject: "test".to_string(),
        body: "test".to_string(),
        currency: "cny".to_string(),
        time_expire: 1717942366,
    };

    let credential_object = match charge_req_payload.channel {
        PaymentChannel::AlipayPcDirect => {
            alipay::AlipayPcDirect::create_credential(&order, &charge_req_payload, &notify_url)
        }
        PaymentChannel::AlipayWap => {
            alipay::AlipayWap::create_credential(&order, &charge_req_payload, &notify_url)
        }
        _ => {
            tracing::error!("create_charge: unsupported channel");
            return Err(StatusCode::BAD_REQUEST);
        }
    };

    let credential_object = credential_object.map_err(|_| {
        tracing::error!("create_credential failed");
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    let mut result = serde_json::to_value(order).map_err(|e| {
        tracing::error!("error serializing create_charge response: {:?}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    result["charge_essentials"] = {
        let mut charge = json!({
            "channel": charge_req_payload.channel,
            "extra": charge_req_payload.extra,
        });
        charge["credential"] = {
            let mut credential = json!({
                "object": "credential",
            });
            let key = serde_json::to_value(&charge_req_payload.channel)
                .unwrap()
                .as_str()
                .unwrap()
                .to_owned();
            credential[key] = credential_object;
            credential
        };
        charge
    };

    Ok(Json(result))
}

pub fn get_routes() -> Router {
    Router::new()
        .route("/test", get(test))
        .route("/orders/:order_id/pay", post(create_charge))
}
