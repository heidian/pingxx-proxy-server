use axum::{
    extract::{Path, Query},
    http::HeaderMap,
    http::StatusCode,
};

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
        .create(charge_id, charge_notify_payload.clone(), vec![])
        .exec()
        .await
        .map_err(|e| {
            tracing::error!("sql error: {:?}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;
    Ok("success".to_string())
}
