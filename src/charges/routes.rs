use axum::{
    extract::Query,
    http::HeaderMap,
    routing::{get, post},
    Router,
};
use rand::Rng;

use super::charge::create_charge;
use super::notify::create_charge_notify;
use super::order::{create_order, retrieve_order};

async fn test() -> String {
    let charge_id = {
        let mut rng = rand::thread_rng();
        let timestamp = chrono::Utc::now().timestamp_millis();
        let number: u64 = rng.gen_range(10000000000..100000000000);
        format!("ch_{}{}", timestamp, number)
    };
    charge_id
}

pub fn get_routes() -> Router {
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
        .route("/v1/orders", post(create_order))
        .route("/v1/orders/:order_id", get(retrieve_order))
        .route("/v1/orders/:order_id/pay", post(create_charge))
        .route("/notify/charges/:charge_id", post(create_charge_notify))
}
