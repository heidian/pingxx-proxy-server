use axum::{extract::Path, http::StatusCode, response::Json};
use rand::Rng;
use serde::{Deserialize, Serialize};
use serde_json::json;

use super::{
    alipay::{self, AlipayPcDirectConfig, AlipayWapConfig},
    order::{load_order_from_db, OrderResponsePayload},
};

#[derive(Deserialize, Serialize, Debug)]
pub enum PaymentChannel {
    #[serde(rename = "alipay_pc_direct")]
    AlipayPcDirect,
    #[serde(rename = "alipay_wap")]
    AlipayWap,
    #[serde(rename = "wx_pub")]
    WxPub,
}

#[derive(Deserialize, Serialize, Debug)]
pub struct ChargeExtra {
    pub success_url: Option<String>,
    pub cancel_url: Option<String>,
}

#[derive(Deserialize, Serialize, Debug)]
pub struct CreateChargeRequestPayload {
    pub charge_amount: u32,
    pub channel: PaymentChannel,
    pub extra: ChargeExtra,
}

async fn load_channel_params_from_db(
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

pub async fn create_charge(
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

    let (order, app, sub_app) = load_order_from_db(&prisma_client, &order_id).await?;

    let credential_object = match charge_req_payload.channel {
        PaymentChannel::AlipayPcDirect => {
            let config =
                load_channel_params_from_db(&prisma_client, sub_app.id, "alipay_pc_direct").await?;
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
            let config =
                load_channel_params_from_db(&prisma_client, sub_app.id, "alipay_wap").await?;
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
