use axum::{
    extract::{Path, Query},
    http::HeaderMap,
    http::StatusCode,
};
use std::str::FromStr;

use super::alipay::{self, AlipayPcDirectConfig, AlipayTradeStatus, AlipayWapConfig};
use super::charge::{load_channel_params_from_db, PaymentChannel};

pub async fn verify(
    prisma_client: &crate::prisma::PrismaClient,
    charge_id: &str,
    payload: &str,
) -> Result<(), StatusCode> {
    let charge = prisma_client
        .charge()
        .find_unique(crate::prisma::charge::charge_id::equals(charge_id.into()))
        .with(crate::prisma::charge::order::fetch().with(crate::prisma::order::sub_app::fetch()))
        .exec()
        .await
        .map_err(|e| {
            tracing::error!("sql error: {:?}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?
        .ok_or_else(|| {
            tracing::error!("charge not found: {}", &charge_id);
            StatusCode::NOT_FOUND
        })?;
    let order = *charge.order.clone().ok_or_else(|| {
        tracing::error!("order not found for charge {}", &charge_id);
        StatusCode::NOT_FOUND
    })?;
    let sub_app = *order.sub_app.clone().ok_or_else(|| {
        tracing::error!("sub_app not found for charge {}", &charge_id);
        StatusCode::NOT_FOUND
    })?;
    let channel = PaymentChannel::from_str(&charge.channel).map_err(|e| {
        tracing::error!("error parsing charge channel: {:?}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    let channel_params = load_channel_params_from_db(prisma_client, sub_app.id, &channel).await?;
    let trade_status = match channel {
        PaymentChannel::AlipayPcDirect => {
            let config = serde_json::from_value::<AlipayPcDirectConfig>(channel_params.params)
                .map_err(|e| {
                    tracing::error!("error deserializing alipay_pc_direct config: {:?}", e);
                    StatusCode::INTERNAL_SERVER_ERROR
                })?;
            alipay::AlipayPcDirect::process_notify(config, payload).map_err(|e| {
                tracing::error!("error processing alipay notify: {:?}", e);
                StatusCode::INTERNAL_SERVER_ERROR
            })?
        }
        PaymentChannel::AlipayWap => {
            let config = serde_json::from_value::<AlipayWapConfig>(channel_params.params)
                .map_err(|e| {
                    tracing::error!("error deserializing alipay_wap config: {:?}", e);
                    StatusCode::INTERNAL_SERVER_ERROR
                })?;
            alipay::AlipayWap::process_notify(config, payload).map_err(|e| {
                tracing::error!("error processing alipay notify: {:?}", e);
                StatusCode::INTERNAL_SERVER_ERROR
            })?
        }
        _ => {
            tracing::error!("unsupported channel: {:?}", channel);
            return Err(StatusCode::BAD_REQUEST);
        }
    };

    if trade_status == AlipayTradeStatus::TradeSuccess || trade_status == AlipayTradeStatus::TradeFinished {
        // update order.paid
        prisma_client
            .order()
            .update(
                crate::prisma::order::id::equals(order.id),
                vec![
                    crate::prisma::order::paid::set(true),
                    crate::prisma::order::time_paid::set(Some(
                        chrono::Utc::now().timestamp() as i32
                    )),
                    crate::prisma::order::amount_paid::set(charge.amount),
                    crate::prisma::order::status::set("paid".to_string()),
                ],
            )
            .exec()
            .await
            .map_err(|e| {
                tracing::error!("sql error: {:?}", e);
                StatusCode::INTERNAL_SERVER_ERROR
            })?;
    }

    Ok(())
}

pub async fn create_charge_notify(
    Query(query): Query<serde_json::Value>,
    Path(charge_id): Path<String>,
    headers: HeaderMap,
    charge_notify_payload: String,
) -> Result<String, StatusCode> {
    let headers_str = format!("{:?}", headers);
    tracing::info!(
        charge_id = charge_id,
        query = query.to_string(),
        payload = charge_notify_payload.as_str(),
        headers = &headers_str,
        "create_charge_notify"
    );
    let prisma_client = crate::prisma::new_client().await.map_err(|e| {
        tracing::error!("error getting prisma client: {:?}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;
    prisma_client
        .charge_notify_history()
        .create(charge_id.clone(), charge_notify_payload.clone(), vec![])
        .exec()
        .await
        .map_err(|e| {
            tracing::error!("sql error: {:?}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    verify(&prisma_client, &charge_id, &charge_notify_payload).await?;

    Ok("success".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_charge_notify_verify_rsa2() {
        tracing_subscriber::fmt::init(); // run test with RUST_LOG=info
        let charge_id = "ch_171792765461736635292532";

        let prisma_client = crate::prisma::new_client().await.unwrap();
        let history = prisma_client
            .charge_notify_history()
            .find_first(vec![
                crate::prisma::charge_notify_history::charge_id::equals(charge_id.into()),
            ])
            .exec()
            .await
            .unwrap()
            .unwrap();

        let payload = history.data.clone();
        verify(&prisma_client, charge_id, &payload).await.unwrap();
    }
}
