use axum::{
    extract::Path,
    http::StatusCode,
    response::Json,
    routing::{get, post},
    Router,
};
use rand::Rng;
use serde_json::json;

use crate::orders::OrderResponsePayload;

mod alipay;
mod charge;
use alipay::{AlipayPcDirectConfig, AlipayWapConfig};
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

async fn get_channel_params(
    prisma_client: &crate::prisma::PrismaClient,
    sub_app_id: i32,
    channel: &str,
) -> Result<crate::prisma::channel_params::Data, StatusCode> {
    let config = prisma_client
        .channel_params()
        .find_unique(crate::prisma::channel_params::sub_app_id_channel(
            sub_app_id,
            String::from(channel),
        ))
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
    Ok(config)
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

    let prisma_client = crate::prisma::new_client().await.map_err(|e| {
        tracing::error!("error getting prisma client: {:?}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    let order = prisma_client
        .order()
        .find_unique(crate::prisma::order::order_id::equals(order_id))
        .with(crate::prisma::order::sub_app::fetch())
        .with(crate::prisma::order::app::fetch())
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
        (app, sub_app)
    };

    let credential_object = match charge_req_payload.channel {
        PaymentChannel::AlipayPcDirect => {
            let config = get_channel_params(&prisma_client, sub_app.id, "alipay_pc_direct").await?;
            let config =
                serde_json::from_value::<AlipayPcDirectConfig>(config.params).map_err(|e| {
                    tracing::error!("error deserializing alipay_pc_direct config: {:?}", e);
                    StatusCode::INTERNAL_SERVER_ERROR
                })?;
            alipay::AlipayPcDirect::create_credential(
                config,
                &order,
                &charge_req_payload,
                &notify_url,
            )
        }
        PaymentChannel::AlipayWap => {
            let config = get_channel_params(&prisma_client, sub_app.id, "alipay_wap").await?;
            let config = serde_json::from_value::<AlipayWapConfig>(config.params).map_err(|e| {
                tracing::error!("error deserializing alipay_wap config: {:?}", e);
                StatusCode::INTERNAL_SERVER_ERROR
            })?;
            alipay::AlipayWap::create_credential(config, &order, &charge_req_payload, &notify_url)
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

    let result = OrderResponsePayload::new(&order, &app, &sub_app);

    let mut result = serde_json::to_value(result).map_err(|e| {
        tracing::error!("error serializing order response payload: {:?}", e);
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
