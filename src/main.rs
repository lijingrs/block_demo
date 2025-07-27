use std::time::Duration;
use axum::body::Body;
use axum::extract::Request;
use axum::Router;
use axum::routing::{get,post};
use nanoid::nanoid;
use tower_http::timeout;
use tower_http::trace::TraceLayer;
use block_demo::application::chat_service;
use block_demo::infra::log::Logger;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let _guard = Logger::init();
    let _ = tokio::join!(axum_start());
    Ok(())
}

async fn axum_start() -> Result<(),anyhow::Error> {
    let host = "0.0.0.0";
    let port = "8080";
    let addr = &format!("{host}:{port}");
    let app = app();
    let listener = tokio::net::TcpListener::bind(addr)
        .await?;
    tracing::info!("listening on {}", listener.local_addr()?);
    let _ = axum::serve(listener, app).await?;
    Ok(())
}

fn app() -> Router {
    let router = router_config();
    router
        .layer(
            TraceLayer::new_for_http()
                .make_span_with(|_request: &Request<Body>| {
                    tracing::error_span!("", "trace_id" = nanoid!())
                }),
        )
        .layer(timeout::TimeoutLayer::new(Duration::from_secs(180)))
}
pub fn router_config() -> Router {
    Router::new()
        .nest("/api", api_router())
}

pub fn api_router() -> Router {
    Router::new()
        .route("/chat/svg_chat", post(chat_service::svg_chat))
        .route("/chat/test", get(chat_service::test))
}