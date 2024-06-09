use axum::{
    extract::{Path, Query},
    http::HeaderMap,
    http::StatusCode,
};

use super::alipay::{verify_rsa2_sign, AlipayPcDirectConfig};

pub async fn verify(
    prisma_client: &crate::prisma::PrismaClient,
    charge_id: &str,
    payload: &str,
) -> Result<bool, StatusCode> {
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

    let channel_params = prisma_client
        .channel_params()
        .find_first(vec![
            crate::prisma::channel_params::sub_app_id::equals(sub_app.id),
            crate::prisma::channel_params::channel::equals(charge.channel.clone()),
        ])
        .exec()
        .await
        .map_err(|e| {
            tracing::error!("sql error: {:?}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?
        .ok_or_else(|| {
            tracing::error!(
                "channel_params not found for sub app {} channel {}",
                &sub_app.key,
                &charge.channel
            );
            StatusCode::NOT_FOUND
        })?;

    let params =
        serde_json::from_value::<AlipayPcDirectConfig>(channel_params.params).map_err(|e| {
            tracing::error!("error deserializing channel_params: {:?}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;
    let public_key = params.alipay_public_key_rsa2.clone();
    let payload = payload.to_string();

    let result = verify_rsa2_sign(&payload, &public_key).map_err(|e| {
        tracing::error!("error verifying rsa2 sign: {:?}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    if result {
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

    Ok(result)
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

    let verified = verify(&prisma_client, &charge_id, &charge_notify_payload).await?;
    if verified {
        Ok("success".to_string())
    } else {
        tracing::error!("verify failed for charge {}", &charge_id);
        Err(StatusCode::BAD_REQUEST)
    }
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
