use axum::{
    routing::{get, post},
    Router,
};
use rand::Rng;

use super::charge::create_charge;
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
        .route("/orders", post(create_order))
        .route("/orders/:order_id", get(retrieve_order))
        .route("/orders/:order_id/pay", post(create_charge))
}
