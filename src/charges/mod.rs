use axum::{
    http::StatusCode,
    response::Json,
    routing::{get, post},
    Router,
};
use serde_json::json;

mod alipay;
mod charge;

use charge::{CreateChargeRequestPayload, PaymentChannel};

async fn test() -> &'static str {
    "ok"
}

async fn create_charge(body: String) -> Result<Json<serde_json::Value>, StatusCode> {
    // tracing::info!("create_charge: {}", body);
    let req_payload: CreateChargeRequestPayload = serde_json::from_str(&body).map_err(|e| {
        tracing::error!("create_charge: {}", e);
        StatusCode::BAD_REQUEST
    })?;
    let notify_url = "https://notify.pingxx.com/notify/charges/ch_101240601691280343040013";
    tracing::info!("create_charge: {:?}", req_payload);

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

    let mut credential = json!({
        "object": "credential",
    });
    credential["alipay_pc_direct"] = credential_object;

    let charge = json!({
        "id": "ch_101240601691280343040013",
        "object": "charge",
        "created": 1717238707,
        "livemode": true,
        "paid": false,
        "refunded": false,
        "reversed": false,
        "app": "app_qnnT0KyzvbfDaT0e",
        "channel": "alipay_pc_direct",
        "order_no": "98520240601184136264",
        "client_ip": "192.168.65.1",
        "amount": 800,
        "amount_settle": 800,
        "currency": "cny",
        "subject": "鬼骨孖的店铺",
        "body": "宝蓝色绑带高跟凉鞋",
        "extra": {
          "success_url": "https://dxd1234.heidianer.com/order/be4570fbd7bf99d17f3b68589a5a46c2a7b302c8?payment_status=success"
        },
        "time_paid": null,
        "time_expire": 1717240296,
        "time_settle": null,
        "transaction_no": null,
        "refunds": {
          "object": "list",
          "url": "/v1/charges/ch_101240601691280343040013/refunds",
          "has_more": false,
          "data": []
        },
        "amount_refunded": 0,
        "failure_code": null,
        "failure_msg": null,
        "metadata": {},
        "credential": credential,
        "description": null
    });
    Ok(Json(charge))
}

pub fn get_routes() -> Router {
    Router::new()
        .route("/test", get(test))
        .route("/charges", post(create_charge))
}
