use axum::{
    http::StatusCode,
    response::Json,
    routing::{get, post},
    Router,
};
use rand::Rng;
use serde_json::json;

mod alipay;
mod charge;

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

async fn create_charge(body: String) -> Result<Json<serde_json::Value>, StatusCode> {
    tracing::info!("create_charge: {}", body);
    let req_payload: CreateChargeRequestPayload = serde_json::from_str(&body).map_err(|e| {
        tracing::error!("error parsing create_charge request payload: {:?}", e);
        StatusCode::BAD_REQUEST
    })?;
    let timestamp = chrono::Utc::now().timestamp_millis();
    let charge_id = {
        let mut rng = rand::thread_rng();
        let number: u64 = rng.gen_range(10000000000..100000000000);
        format!("ch_{}{}", timestamp, number)
    };

    let charge_notify_url_root = std::env::var("CHARGE_NOTIFY_URL_ROOT").unwrap();
    let notify_url = format!("{}{}", charge_notify_url_root, charge_id);
    // "https://notify.pingxx.com/notify/charges/ch_101240601691280343040013";

    let credential_object = match req_payload.channel {
        PaymentChannel::AlipayPcDirect => {
            alipay::AlipayPcDirect::create_credential(&req_payload, &notify_url)
        }
        PaymentChannel::AlipayWap => {
            alipay::AlipayWap::create_credential(&req_payload, &notify_url)
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

    let mut charge = json!({
        "id": charge_id,
        "object": "charge",
        "created": timestamp,
        "livemode": true,
        "paid": false,
        // "refunded": false,
        "reversed": false,
        "time_paid": null,
        "time_settle": null,
        "transaction_no": null,
        // "refunds": {
        //     "object": "list",
        //     "url": "/v1/charges/ch_101240601691280343040013/refunds",
        //     "has_more": false,
        //     "data": []
        // },
        // "amount_refunded": 0,
        "failure_code": null,
        "failure_msg": null,
        "metadata": {},
        "description": null
    });

    charge.as_object_mut().unwrap().extend({
        let req_payload_obj = serde_json::to_value(&req_payload).map_err(|e| {
            tracing::error!("error serializing create_charge response: {:?}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;
        req_payload_obj.as_object().unwrap().clone()
    });

    charge["credential"] = {
        let mut credential = json!({
            "object": "credential",
        });
        let key = serde_json::to_value(&req_payload.channel)
            .unwrap()
            .as_str()
            .unwrap()
            .to_owned();
        credential[key] = credential_object;
        credential
    };

    Ok(Json(charge))
}

pub fn get_routes() -> Router {
    Router::new()
        .route("/test", get(test))
        .route("/charges", post(create_charge))
}
