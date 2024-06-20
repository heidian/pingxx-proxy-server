mod charge;
mod notify;
mod order;
use axum::{
    extract::{Path, Query},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Json},
    routing::{get, post},
    Router,
};
use charge::{create_charge, CreateChargeRequestPayload};
use notify::{create_charge_notify, retry_charge_notify};
use order::{create_order, retrieve_order, CreateOrderRequestPayload};

async fn test() -> &'static str {
    "test"
}

pub async fn get_routes() -> Router {
    let prisma_client = crate::prisma::new_client()
        .await
        .expect("error getting prisma client");
    let prisma_client = std::sync::Arc::new(prisma_client);

    Router::new()
        .route("/test", get(test))
        .route(
            "/.ping",
            post({
                |Query(query): Query<serde_json::Value>, headers: HeaderMap, payload: String| async move {
                    tracing::info!(
                        query = query.to_string(),
                        payload = payload,
                        headers = &format!("{:?}", headers),
                        ".ping"
                    );
                    "pingxx ok"
                }
            }).merge(get({
                |Query(query): Query<serde_json::Value>, headers: HeaderMap| async move {
                    tracing::info!(
                        query = query.to_string(),
                        headers = &format!("{:?}", headers),
                        ".ping"
                    );
                    "pingxx ok"
                }
            })),
        )
        .route("/v1/orders", {
            let prisma_client = prisma_client.clone();
            post(|body: String| async move {
                tracing::info!(body, "create_order");
                let payload: CreateOrderRequestPayload = serde_json::from_str(&body).map_err(|e| {
                    let err_msg = format!("error parsing create_order request payload: {:?}", e);
                    (StatusCode::BAD_REQUEST, err_msg).into_response()
                })?;
                match create_order(&prisma_client, payload).await {
                    Ok(result) => Ok(Json(result)),  // 确认下这里的 Json 会 panic 吗
                    Err(error) => Err(error.into_response()),
                }
            })
        })
        .route("/v1/orders/:order_id", {
            let prisma_client = prisma_client.clone();
            get(|Path(order_id): Path<String>| async move {
                match retrieve_order(&prisma_client, order_id).await {
                    Ok(result) => Ok(Json(result)),
                    Err(error) => Err(error.into_response()),
                }
            })
        })
        .route("/v1/orders/:order_id/pay", {
            let prisma_client = prisma_client.clone();
            post(|
                Path(order_id): Path<String>,
                body: String,
                // Json(charge_req_payload): Json<CreateChargeRequestPayload>,
            | async move {
                tracing::info!(order_id, body, "create_charge");
                let charge_req_payload: CreateChargeRequestPayload =
                    serde_json::from_str(&body).map_err(|e| {
                        let err_msg = format!("error parsing create_charge request payload: {:?}", e);
                        (StatusCode::BAD_REQUEST, err_msg).into_response()
                    })?;
                match create_charge(&prisma_client, order_id, charge_req_payload).await {
                    Ok(result) => Ok(Json(result)),
                    Err(error) => Err(error.into_response()),
                }
            })
        })
        .route("/notify/charges/:charge_id", {
            let prisma_client = prisma_client.clone();
            post(|
                Query(query): Query<serde_json::Value>,
                Path(charge_id): Path<String>,
                headers: HeaderMap,
                body: String
            | async move {
                let headers_str = format!("{:?}", headers);
                tracing::info!(
                    charge_id = charge_id,
                    query = query.to_string(),
                    payload = body.as_str(),
                    headers = &headers_str,
                    "create_charge_notify"
                );
                create_charge_notify(&prisma_client, charge_id, body).await
            })
        })
        .route("/notify/:id/retry", {
            let prisma_client = prisma_client.clone();
            post(|Path(id): Path<i32>| async move {
                retry_charge_notify(&prisma_client, id).await
            })
        })
}
