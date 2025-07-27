use crate::domain::entity;
use crate::domain::service::question_screenshot_domain_service::QuestionScreenshotDomainService;
use crate::infrastructure::config::settings;
use crate::infrastructure::utils::date::DateUtil;
use crate::infrastructure::utils::local_cache::{Expiration, LocalCache};
use crate::infrastructure::utils::redis_util::RedisClient;
use crate::presentation::error::ApiErr;
use crate::presentation::result::CommonResult;
use async_openai::error::OpenAIError::ApiError;
use base64::Engine;
use headless_chrome::protocol::cdp::Page;
use headless_chrome::protocol::cdp::Page::CaptureScreenshotFormatOption;
use headless_chrome::types::Bounds;
use headless_chrome::{Browser, LaunchOptions};
use once_cell::sync::Lazy;
use sea_orm::{Iden, Set};
use std::ffi::OsStr;
use std::sync::{Arc, Mutex};
use std::time::Instant;
use tracing::log::debug;
use tracing::{error, info};

pub struct ScreenShotUtil {}

static BROWSER_POOL: Lazy<Mutex<Option<Arc<Browser>>>> = Lazy::new(|| {
    Mutex::new(None)
});
impl ScreenShotUtil {
    pub fn screenshot(url: &str,selector:&str) -> CommonResult<String> {
        let browser = get_or_create_browser()?;
        let tab = browser.new_tab()?;
        tab.navigate_to(url)?.wait_until_navigated()?;
        let box_model = tab.find_element(selector)?.get_box_model()?;
        let mut body_height = box_model.height;
        let body_height2 = tab.find_element("#question-list")?.get_box_model()?.height;
        if body_height2 > body_height {
            debug!("使用question-list的长度");
            body_height = body_height2;
        }
        let body_width = box_model.width;
        info!("截图尺寸：长：{},宽：{}", body_height,body_width);
        tab.set_bounds(Bounds::Normal { left: Some(0), top: Some(0), width:Some(body_width), height: Some(body_height) })?;
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
    // 测试题目： 6823920 11042344 11442902 15036927 6774542(长度不够 答案未截取 修改长度为920)
    async fn screenshot_local(url: &str,selector:&str) -> CommonResult<Vec<u8>> {
        let browser = get_or_create_browser()?;
        let tab = browser.new_tab()?;
        tab.navigate_to(url)?
            .wait_until_navigated()?;
        let mut body_height = tab.find_element(selector)?.get_box_model()?.height;
        let body_width = tab.find_element(selector)?.get_box_model()?.width;
        let body_height2 = tab.find_element("#question-list")?.get_box_model()?.height;
        if body_height2 > body_height {
            info!("使用question-list的长度");
            body_height = body_height2;
        }
        info!("截图尺寸：长：{}-宽：{}", body_height,body_width);
        tab.set_bounds(Bounds::Normal { left: Some(0), top: Some(0), width:Some(body_width), height: Some(body_height) })?;
        let data = tab
            .call_method(Page::CaptureScreenshot {
                format: Some(CaptureScreenshotFormatOption::Png),
                clip:None,
                quality:Some(100),
                from_surface: Some(true),
                capture_beyond_viewport: Some(true),
                optimize_for_speed: Some(true),
            })?
            .data;
       let jpeg_data =  base64::prelude::BASE64_STANDARD
            .decode(data).expect("base64 decode failed");
        tab.close_target()?;
        Ok(jpeg_data)
    }
    pub async fn question_screenshot(question_id:u64) -> CommonResult<String> {
        let key = format!("question_screenshot:{}", question_id);
        if let Some((_,image_base64)) = LocalCache::get(&key).await{
            LocalCache::insert(key, (Expiration::Seconds60, image_base64.clone())).await;
            return Ok(image_base64);
        }
        let cache = RedisClient::global().get_string(&key).await?;
        if let Some(base64_str) = cache{
            let image_base64 = format!(
                "data:image/png;base64,{}",base64_str
            );
            return Ok(image_base64);
        }
        // 缓存没有，查询数据库是否存在
        let db_cache = QuestionScreenshotDomainService::find_by_id(question_id).await?;
        if let Some(db_data) = db_cache{
            let base64_str =  base64::prelude::BASE64_STANDARD
                .encode(db_data.binary_data);
            let image_base64 = format!(
                "data:image/png;base64,{}",base64_str
            );
            // Redis缓存
            RedisClient::global().set_string(&key,&base64_str.clone(),120).await?;
            return Ok(image_base64);
        }
        let domain = settings::global().get_string("question_preview_host").expect("question_preview_host not set");
        let url = format!("{}/#/questionpreview?questionId={}", domain, question_id);
        let selector = ".quiz-display";
        let start = Instant::now();
        let current_span = tracing::Span::current();
        let base64_str = tokio::task::spawn_blocking(move||{
            current_span.in_scope(||ScreenShotUtil::screenshot(&url,selector))
            }).await??;
        tokio::task::yield_now().await;
        let image_base64 = format!(
            "data:image/png;base64,{}",base64_str
        );
        tokio::spawn(async move {
            let question_id = question_id.clone();
            let entity = entity::entity_question_screenshot::ActiveModel{
                question_id: Set(question_id),
                binary_data: Set(base64::prelude::BASE64_STANDARD.decode(base64_str.clone()).expect("base64 decode failed")),
                create_time: Set(DateUtil::now()),
                update_time: Set(DateUtil::now()),
            };
            let _ = QuestionScreenshotDomainService::upsert(entity).await;
        });
        info!("题目:{},截图耗时：{:?}",question_id,start.elapsed());
        LocalCache::insert(key, (Expiration::Seconds60, image_base64.clone())).await;
        Ok(image_base64)
    }
    pub async fn question_screenshot_local(question_id:u64) -> CommonResult<Vec<u8>> {
        let domain = settings::global().get_string("question_preview_host").expect("question_preview_host not set");
        let url = &format!("{}/#/questionpreview?questionId={}", domain, question_id);
        let selector = ".quiz-display";
        let start = Instant::now();
        let screen_result = ScreenShotUtil::screenshot_local(url,selector).await;
        match screen_result {
            Ok(data) => {
                info!("题目:{},截图耗时：{:?}",question_id,start.elapsed());
                Ok(data)
            }
            Err(err) => {
                error!("截屏失败:{:?}",err);
                Err(err)
            }
        }
    }
}

fn get_or_create_browser() -> CommonResult<Arc<Browser>> {
    let mut browser_guard = BROWSER_POOL.lock().expect("创建浏览器失败");

    if let Some(ref browser) = *browser_guard {
        if browser.get_version().is_ok() {
            return Ok(Arc::clone(browser));
        }
    }
    // 创建新的浏览器实例
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
#[cfg(test)]
mod tests {
    use crate::infrastructure::config::nacos::NacosConfigListener;
    use crate::infrastructure::utils::screen_shot_util::ScreenShotUtil;

    #[tokio::test]
    pub async fn test_screen_shot(){
        use crate::infrastructure::config::settings;
        use crate::infrastructure::config::log;
        let _ = settings::Settings::init().await;
        let _ = log::Logger::init();
        let _ = NacosConfigListener::init().await;
        let question_id = 11042344;
        let result = ScreenShotUtil::question_screenshot_local(question_id).await.unwrap();
    }
}