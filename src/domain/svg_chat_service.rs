use std::time::{Duration};
use tracing::{error, warn};
use crate::application::ao::SVGListAO;
use crate::infra::screen_shot_util::ScreenShotUtil;
use crate::infra::svg_util::SvgUtil;

pub struct SVGChatService{}

impl SVGChatService {
    pub async fn process_task_with_retry(ao: SVGListAO) -> bool{
        for retry in 0..3 {
            match Self::process_async(ao.clone()).await {
                Ok(_) => {
                    return true;
                },
                Err(e) if retry < 2 => {
                    warn!("retry {}: {:?}", retry + 1, e);
                    tokio::time::sleep(Duration::from_millis(100)).await;
                }
                Err(e) => {
                    error!("process_task_with_retry ERROR: {:?}", e);
                }
            }
        }
        false
    }
    async fn process_async(ao: SVGListAO) -> Result<(),anyhow::Error> {
        let _ = Self::process_single(ao).await?;
        Ok(())
    }
    async fn process_single(
        ao: SVGListAO,
    ) -> Result<(),anyhow::Error> {
        Self::exec_single_processor(&ao).await
    }
    async fn exec_single_processor(
        svg_list: &SVGListAO,
    ) -> Result<(),anyhow::Error> {
        let _svg_results = SvgUtil::batch_process_svgs(svg_list.svg_list.clone()).await?;
        let _ai_result =  Self::call_ai_service().await;
        Ok(())
    }
    async fn call_ai_service(
    ) -> Result<String,anyhow::Error> {
        let _question_image_base64 = ScreenShotUtil::question_screenshot(1).await?;
        // mock call ai
        tokio::time::sleep(Duration::from_secs(40)).await;
        Ok("success".to_string())
    }
}





