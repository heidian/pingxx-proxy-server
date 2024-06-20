// #![recursion_limit = "256"]
use axum::{routing::get, Router};
use dotenvy::dotenv;
use tower::ServiceBuilder;
use tower_http::trace::TraceLayer;

mod alipay;
mod routes;
mod core;
#[allow(dead_code, unused_imports)]
mod prisma;
mod utils;
mod weixin;

#[tokio::main]
async fn main() {
    dotenv().ok();
    tracing_subscriber::fmt::init();
    // build our application with a route
    let charge_routes = routes::get_routes().await;
    let app = Router::new()
        .nest("/", charge_routes)
        .route("/", get(root))
        .layer(
            ServiceBuilder::new()
                .layer(TraceLayer::new_for_http())
                .into_inner(),
        );

    // run our app with hyper, listening globally on port 3000
    let listener = tokio::net::TcpListener::bind("0.0.0.0:8002").await.unwrap();
    axum::serve(listener, app).await.unwrap();
}

// basic handler that responds with a static string
async fn root() -> &'static str {
    "Hello, World!"
}
