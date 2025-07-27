use headless_chrome::protocol::cdp::Page;
use headless_chrome::protocol::cdp::Page::CaptureScreenshotFormatOption;
use headless_chrome::types::Bounds;
use headless_chrome::{Browser, LaunchOptions};
use once_cell::sync::Lazy;
use std::ffi::OsStr;
use std::sync::{Arc, Mutex};
use base64::Engine;
use crate::infra::local_cache_util::{Expiration, LocalCache};

pub struct ScreenShotUtil {}

static BROWSER_POOL: Lazy<Mutex<Option<Arc<Browser>>>> = Lazy::new(|| {
    Mutex::new(None)
});
impl ScreenShotUtil {
    pub fn screenshot(url: &str,_selector:&str) -> Result<String,anyhow::Error> {
        let browser = get_or_create_browser()?;
        let tab = browser.new_tab()?;
        tab.navigate_to(url)?.wait_until_navigated()?;
        tab.set_bounds(Bounds::Normal { left: Some(0), top: Some(0), width:Some(920f64), height: Some(740f64) })?;
        let data = tab
            .call_method(Page::CaptureScreenshot {
                format: Some(CaptureScreenshotFormatOption::Png),
                clip:None,
                quality:Some(100),
                from_surface: Some(true),
                capture_beyond_viewport: Some(true),
                optimize_for_speed: Some(true),
            })?.data;
        tokio::spawn(async move {
            let _ = tab.close_target();
        });
        Ok(data)
    }

    pub async fn question_screenshot(question_id:u64) -> Result<String,anyhow::Error> {
        let key = format!("question_screenshot:{}", question_id);
        if let Some((_,image_base64)) = LocalCache::get(&key).await{
            LocalCache::insert(key, (Expiration::Seconds60, image_base64.clone())).await;
            return Ok(image_base64);
        }
        // redis cache
        let cache: std::option::Option<String> = None;
        if let Some(base64_str) = cache{
            let image_base64 = format!(
                "data:image/png;base64,{}",base64_str
            );
            return Ok(image_base64);
        }
        // db cache sea-orm query
        // pool timed out while waiting for an open connection
        // let db_cache = QuestionScreenshotDomainService::find_by_id(question_id).await?;
        let db_cache: std::option::Option<String> = None;
        if let Some(db_data) = db_cache{
            let base64_str =  base64::prelude::BASE64_STANDARD
                .encode(db_data);
            let image_base64 = format!(
                "data:image/png;base64,{}",base64_str
            );
            // Redis cache
            // RedisClient::global().set_string(&key,&base64_str.clone(),120).await?;
            return Ok(image_base64);
        }
        let domain = "www.google.com";
        let url = format!("{}/#?questionId={}", domain, question_id);
        let selector = ".quiz-display";
        let current_span = tracing::Span::current();
        let base64_str = tokio::task::spawn_blocking(move||{
            current_span.in_scope(||ScreenShotUtil::screenshot(&url,selector))
        }).await??;
        tokio::task::yield_now().await;
        let image_base64 = format!(
            "data:image/png;base64,{}",base64_str
        );
        tokio::spawn(async move {
            // let question_id = question_id.clone();
            // let entity = entity::entity_question_screenshot::ActiveModel{
            //     question_id: Set(question_id),
            //     binary_data: Set(base64::prelude::BASE64_STANDARD.decode(base64_str.clone()).expect("base64 decode failed")),
            //     create_time: Set(DateUtil::now()),
            //     update_time: Set(DateUtil::now()),
            // };
            // let _ = QuestionScreenshotDomainService::upsert(entity).await;
        });
        LocalCache::insert(key, (Expiration::Seconds60, image_base64.clone())).await;
        Ok(image_base64)
    }
}

fn get_or_create_browser() -> Result<Arc<Browser>,anyhow::Error> {
    let mut browser_guard = BROWSER_POOL.lock().expect("get browser failed");

    if let Some(ref browser) = *browser_guard {
        if browser.get_version().is_ok() {
            return Ok(Arc::clone(browser));
        }
    }
    let options = LaunchOptions::default_builder()
        .args(vec![
            OsStr::new("--no-sandbox"),
            OsStr::new("--disable-setuid-sandbox"),
            OsStr::new("--disable-dev-shm-usage"),
            OsStr::new("--disable-gpu"),
            OsStr::new("--hide-scrollbars"),
            OsStr::new("--disable-web-security"),
            OsStr::new("--disable-features=VizDisplayCompositor"),
        ])
        .window_size(Some((820,920)))
        .sandbox(false)
        .build()?;

    let browser = Arc::new(Browser::new(options)?);
    *browser_guard = Some(Arc::clone(&browser));
    Ok(browser)
}