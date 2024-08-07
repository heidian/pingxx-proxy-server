mod basic;
mod notify;
mod order;
mod prelude;
mod sub_app;
use axum::{
    extract::{Path, Query, Request},
    http::{HeaderMap, StatusCode},
    middleware::{self, Next},
    response::{IntoResponse, Json, Response},
    routing::{get, post, put},
    Router,
};
use notify::{create_charge_notify, create_refund_notify, retry_notify};
use sub_app::{create_or_update_sub_app_channel, retrieve_sub_app};

async fn auth(req: Request, next: Next) -> Result<Response, StatusCode> {
    let api_live_key = std::env::var("API_LIVE_KEY").expect("API_LIVE_KEY is not set");
    let auth_header = req
        .headers()
        .get("Authorization")
        .and_then(|header| header.to_str().ok());

    if let Some(auth_header) = auth_header {
        if auth_header.starts_with("Bearer ") {
            let credential = auth_header.trim_start_matches("Bearer ");
            if credential == api_live_key {
                return Ok(next.run(req).await);
            }
        }
    }
    Err(StatusCode::UNAUTHORIZED)
}

pub async fn get_routes() -> Router {
    let prisma_client = crate::prisma::new_client()
        .await
        .expect("error getting prisma client");
    let prisma_client = std::sync::Arc::new(prisma_client);

    Router::new()
        .route("/v1/orders", {
            let prisma_client = prisma_client.clone();
            post(|body: String| async move {
                tracing::info!(body, "create_order");
                let payload: order::CreateOrderRequestPayload = serde_json::from_str(&body)
                    .map_err(|e| {
                        let err_msg =
                            format!("error parsing create_order request payload: {:?}", e);
                        (StatusCode::BAD_REQUEST, err_msg).into_response()
                    })?;
                match order::create_order(&prisma_client, payload).await {
                    Ok(result) => Ok(Json(result)), // 确认下这里的 Json 会 panic 吗
                    Err(error) => Err(error.into_response()),
                }
            })
        })
        .route("/v1/orders/:order_id", {
            let prisma_client = prisma_client.clone();
            get(|Path(order_id): Path<String>| async move {
                match order::retrieve_order(&prisma_client, order_id).await {
                    Ok(result) => Ok(Json(result)),
                    Err(error) => Err(error.into_response()),
                }
            })
        })
        .route("/v1/orders/:order_id/pay", {
            let prisma_client = prisma_client.clone();
            // post(|Path(order_id): Path<String>, Json(charge_req_payload): Json<order::CreateChargeRequestPayload>| async move {
            post(|Path(order_id): Path<String>, body: String| async move {
                tracing::info!(order_id, body, "create_charge");
                let charge_req_payload: order::CreateChargeRequestPayload =
                    serde_json::from_str(&body).map_err(|e| {
                        let err_msg =
                            format!("error parsing create_charge request payload: {:?}", e);
                        (StatusCode::BAD_REQUEST, err_msg).into_response()
                    })?;
                match order::create_charge(&prisma_client, order_id, charge_req_payload).await {
                    Ok(result) => Ok(Json(result)),
                    Err(error) => Err(error.into_response()),
                }
            })
        })
        .route("/v1/orders/:order_id/order_refunds", {
            let prisma_client = prisma_client.clone();
            post(|Path(order_id): Path<String>, body: String| async move {
                tracing::info!(order_id, body, "create_refund");
                let refund_req_payload: order::CreateRefundRequestPayload =
                    serde_json::from_str(&body).map_err(|e| {
                        let err_msg =
                            format!("error parsing create_refund request payload: {:?}", e);
                        (StatusCode::BAD_REQUEST, err_msg).into_response()
                    })?;
                match order::create_refund(&prisma_client, order_id, refund_req_payload).await {
                    Ok(result) => Ok(Json(result)),
                    Err(error) => Err(error.into_response()),
                }
            })
        })
        .route("/v1/orders/:order_id/order_refunds/:refund_id", {
            let prisma_client = prisma_client.clone();
            get(
                |Path((order_id, refund_id)): Path<(String, String)>| async move {
                    tracing::info!(order_id, refund_id, "retrieve_refund");
                    match order::retrieve_refund(&prisma_client, order_id, refund_id).await {
                        Ok(result) => Ok(Json(result)),
                        Err(error) => Err(error.into_response()),
                    }
                },
            )
        })
        .route("/v1/charges", {
            let prisma_client = prisma_client.clone();
            post(|body: String| async move {
                tracing::info!(body, "create_charge");
                let charge_req_payload: basic::CreateChargeRequestPayload =
                    serde_json::from_str(&body).map_err(|e| {
                        let err_msg =
                            format!("error parsing create_charge request payload: {:?}", e);
                        (StatusCode::BAD_REQUEST, err_msg).into_response()
                    })?;
                match basic::create_charge(&prisma_client, charge_req_payload).await {
                    Ok(result) => Ok(Json(result)),
                    Err(error) => Err(error.into_response()),
                }
            })
        })
        .route("/v1/charges/:charge_id", {
            let prisma_client = prisma_client.clone();
            get(|Path(charge_id): Path<String>| async move {
                match basic::retrieve_charge(&prisma_client, charge_id).await {
                    Ok(result) => Ok(Json(result)),
                    Err(error) => Err(error.into_response()),
                }
            })
        })
        .route("/v1/charges/:charge_id/refunds", {
            let prisma_client = prisma_client.clone();
            post(|Path(charge_id): Path<String>, body: String| async move {
                tracing::info!(charge_id, body, "create_refund");
                let refund_req_payload: basic::CreateRefundRequestPayload =
                    serde_json::from_str(&body).map_err(|e| {
                        let err_msg =
                            format!("error parsing create_refund request payload: {:?}", e);
                        (StatusCode::BAD_REQUEST, err_msg).into_response()
                    })?;
                match basic::create_refund(&prisma_client, charge_id, refund_req_payload).await {
                    Ok(result) => Ok(Json(result)),
                    Err(error) => Err(error.into_response()),
                }
            })
        })
        .route("/v1/charges/:charge_id/refunds/:refund_id", {
            let prisma_client = prisma_client.clone();
            get(
                |Path((charge_id, refund_id)): Path<(String, String)>| async move {
                    tracing::info!(charge_id, refund_id, "retrieve_refund");
                    match basic::retrieve_refund(&prisma_client, charge_id, refund_id).await {
                        Ok(result) => Ok(Json(result)),
                        Err(error) => Err(error.into_response()),
                    }
                },
            )
        })
        .route("/v1/apps/:app_id/sub_apps/:sub_app_id", {
            let prisma_client = prisma_client.clone();
            get(
                |Path((app_id, sub_app_id)): Path<(String, String)>| async move {
                    match retrieve_sub_app(&prisma_client, app_id, sub_app_id).await {
                        Ok(result) => Ok(Json(result)),
                        Err(error) => Err((StatusCode::BAD_REQUEST, error).into_response()),
                    }
                },
            )
        })
        .route("/v1/apps/:app_id/sub_apps/:sub_app_id/channels/:channel", {
            let prisma_client = prisma_client.clone();
            put(
                |Path((app_id, sub_app_id, channel)): Path<(String, String, String)>,
                 Json(payload): Json<serde_json::Value>| async move {
                    let params = payload["params"].clone();
                    match create_or_update_sub_app_channel(
                        &prisma_client,
                        app_id,
                        sub_app_id,
                        channel,
                        params,
                    )
                    .await
                    {
                        Ok(result) => Ok(Json(result)),
                        Err(error) => Err((StatusCode::BAD_REQUEST, error).into_response()),
                    }
                },
            )
        })
        .route("/v1/apps/:app_id/sub_apps/:sub_app_id/channels", {
            let prisma_client = prisma_client.clone();
            post(
                |Path((app_id, sub_app_id)): Path<(String, String)>,
                 Json(payload): Json<serde_json::Value>| async move {
                    let channel = payload["channel"].as_str().unwrap_or_default().to_string();
                    let params = payload["params"].clone();
                    match create_or_update_sub_app_channel(
                        &prisma_client,
                        app_id,
                        sub_app_id,
                        channel,
                        params,
                    )
                    .await
                    {
                        Ok(result) => Ok(Json(result)),
                        Err(error) => Err((StatusCode::BAD_REQUEST, error).into_response()),
                    }
                },
            )
        })
        .layer(middleware::from_fn(auth))
        /*
         * 之后的 route 不需要 bearer auth, 会各自验证不同渠道的签名
         */
        .route("/notify/charges/:charge_id", {
            let prisma_client = prisma_client.clone();
            post(
                |Query(query): Query<serde_json::Value>,
                 Path(charge_id): Path<String>,
                 headers: HeaderMap,
                 body: String| async move {
                    let headers_str = format!("{:?}", headers);
                    tracing::info!(
                        charge_id = charge_id,
                        query = query.to_string(),
                        payload = body.as_str(),
                        headers = &headers_str,
                        "create_charge_notify"
                    );
                    create_charge_notify(&prisma_client, charge_id, body).await
                },
            )
        })
        .route("/notify/charges/:charge_id/refunds/:refund_id", {
            let prisma_client = prisma_client.clone();
            post(
                |Query(query): Query<serde_json::Value>,
                 Path((charge_id, refund_id)): Path<(String, String)>,
                 headers: HeaderMap,
                 body: String| async move {
                    let headers_str = format!("{:?}", headers);
                    tracing::info!(
                        charge_id = charge_id,
                        refund_id = refund_id,
                        query = query.to_string(),
                        payload = body.as_str(),
                        headers = &headers_str,
                        "create_refund_notify"
                    );
                    create_refund_notify(&prisma_client, charge_id, refund_id, body).await
                },
            )
        })
        .route("/notify/:id/retry", {
            let prisma_client = prisma_client.clone();
            post(|Path(id): Path<i32>| async move { retry_notify(&prisma_client, id).await })
        })
}
