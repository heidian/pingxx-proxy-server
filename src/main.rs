// #![recursion_limit = "256"]
use axum::{
    extract::Query,
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::get,
    Router,
};
use dotenvy::dotenv;
use tower::ServiceBuilder;
use tower_http::{
    trace::{DefaultMakeSpan, DefaultOnRequest, DefaultOnResponse, TraceLayer},
    LatencyUnit,
};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

mod alipay;
mod core;
#[allow(dead_code, unused_imports)]
mod prisma;
mod routes;
mod utils;
mod weixin;

#[tokio::main]
async fn main() {
    dotenv().ok();
    // tracing_subscriber::fmt::init();
    tracing_subscriber::registry()
        .with(tracing_subscriber::EnvFilter::from_default_env())
        .with(tracing_subscriber::fmt::layer().with_ansi({
            if cfg!(debug_assertions) {
                true
            } else {
                false
            }
        }))
        .init();
    // build our application with a route
    let charge_routes = routes::get_routes().await;
    let app = Router::new()
        .nest("/", charge_routes)
        .route("/", get(root))
        .fallback(fallback)
        .layer(
            ServiceBuilder::new()
                .layer(
                    TraceLayer::new_for_http()
                        .make_span_with(
                            // 配置 span 的 level 是 DEBUG, 如果环境变量 RUST_LOG=info 则不会生成 span
                            DefaultMakeSpan::new().level(tracing::Level::DEBUG), // .include_headers(true)
                        )
                        .on_request(
                            // 配置 request 和 response 的 log 用 INFO level 输出, 如果环境变量 RUST_LOG=error 则不会输出
                            DefaultOnRequest::new().level(tracing::Level::INFO),
                        )
                        .on_response(
                            DefaultOnResponse::new()
                                .level(tracing::Level::INFO)
                                .latency_unit(LatencyUnit::Micros),
                        ),
                )
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

async fn fallback(Query(query): Query<serde_json::Value>, body: String) -> Response {
    tracing::info!(query = query.to_string(), body = body, "fallback");
    (StatusCode::NOT_FOUND, "not found".to_string()).into_response()
}
