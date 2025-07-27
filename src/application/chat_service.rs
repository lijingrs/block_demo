use axum::http::StatusCode;
use axum::Json;
use axum::response::{IntoResponse};
use tracing::Instrument;
use crate::application::ao::SVGListAO;
use crate::domain::svg_chat_service::SVGChatService;

pub async fn svg_chat(Json(req):Json<SVGListAO>) -> impl IntoResponse{
    let current_span = tracing::Span::current();
    tokio::spawn(async move{
        let _ = SVGChatService::process_task_with_retry(req).await;
    }.instrument(current_span));
    (StatusCode::OK, "ok")
}

pub async fn test() -> impl IntoResponse{
    tokio::time::sleep(tokio::time::Duration::from_millis(200)).await;
    (StatusCode::OK, "ok")
}