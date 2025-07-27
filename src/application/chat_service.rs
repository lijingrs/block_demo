use axum::Json;
use axum::response::{IntoResponse, Response};
use crate::application::ao::SVGListAO;
use crate::application::svg_chat_service;

pub fn svg_chat(Json(req):Json<SVGListAO>) -> Response{
    tokio::spawn(async move{
        let _ = svg_chat_service::process_task_with_retry(req).await;
    });
    Ok(()).into_response()
}