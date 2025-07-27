use std::sync::{Arc, LazyLock};
use std::time::Duration;
use anyhow::anyhow;
use base64::Engine;
use once_cell::sync::OnceCell;
use rayon::iter::IntoParallelRefIterator;
use resvg::{tiny_skia, usvg};
use tokio::sync::Semaphore;
use tokio::task;
use crate::domain::dto::TaskResult;
use rayon::iter::ParallelIterator;
use rayon::iter::IndexedParallelIterator;
use reqwest::Client;

static HTTP_CLIENT: LazyLock<Client> = LazyLock::new(||Client::builder()
    .timeout(Duration::from_secs(30))
    .pool_max_idle_per_host(20)
    .pool_idle_timeout(Duration::from_secs(30))
    .user_agent("svg-processor/1.0")
    .build()
    .expect("Failed to create HTTP client"));

static HTTP_SEMAPHORE: OnceCell<Arc<Semaphore>> = OnceCell::new();
pub struct SvgUtil;
impl SvgUtil {
    pub async fn batch_process_svgs(svg_urls: Vec<String>) -> Result<Vec<String>,anyhow::Error> {
        let tasks = Self::process_suffix_svg(svg_urls).await?;
        let mut results = Vec::new();
        for task in tasks {
            if task.error.is_some() {
                return Err(anyhow!(task.error.unwrap()));
            }
            results.push(task.base64_data);
        }
        Ok(results)
    }

    pub async fn process_suffix_svg(svg_suffix_url:Vec<String>) -> Result<Vec<TaskResult>,anyhow::Error>{
        let download_tasks: Vec<_> = svg_suffix_url
            .into_iter()
            .enumerate()
            .map(|(idx, svg_url)| {
                let client = HTTP_CLIENT.clone();
                let semaphore = HTTP_SEMAPHORE.get_or_init(|| Arc::new(Semaphore::new(50)));

                tokio::spawn(async move {
                    let _permit = semaphore.acquire().await.map_err(|e| {
                        anyhow!(format!("Semaphore error: {}", e))
                    })?;

                    let result = client
                        .get(&svg_url)
                        .send()
                        .await?
                        .error_for_status()?
                        .bytes()
                        .await;

                    match result {
                        Ok(bytes) => {
                            Ok((idx, bytes.to_vec()))
                        }
                        Err(e) => {
                            Err(anyhow!(format!("HTTP error for {}: {}", svg_url, e)))
                        }
                    }
                })
            })
            .collect();
        let mut svg_data_list = vec![None; download_tasks.len()];
        let mut has_error = false;
        let mut error_msg = String::new();
        for task in download_tasks {
            match task.await {
                Ok(Ok((idx, data))) => {
                    svg_data_list[idx] = Some(data);
                }
                Ok(Err(e)) => {
                    has_error = true;
                    error_msg = e.to_string();
                    break;
                }
                Err(e) => {
                    has_error = true;
                    error_msg = format!("Task join error: {}", e);
                    break;
                }
            }
        }
        if has_error {
            return Err(anyhow!(error_msg));
        }
        let valid_svg_data: Vec<(usize, Vec<u8>)> = svg_data_list
            .into_iter()
            .enumerate()
            .filter_map(|(idx, data)| data.map(|d| (idx, d)))
            .collect();
        let result = task::spawn_blocking(move ||Self::process_svg_pipeline(valid_svg_data)).await;
        task::yield_now().await;
        match result {
            Ok(tasks) => Ok(tasks),
            Err(err) => {
                Err(anyhow!(err.to_string()))
            }
        }
    }

    fn process_svg_pipeline(svg_data_list: Vec<(usize, Vec<u8>)>) -> Vec<TaskResult> {
        let parsed_trees: Vec<_> = svg_data_list
            .par_iter()
            .enumerate()
            .map(|(_, (index,svg_data))| {
                let opt = usvg::Options::default();
                match usvg::Tree::from_data(svg_data, &opt) {
                    Ok(tree) => Some((index, tree)),
                    Err(e) => {
                        eprintln!("Failed to parse SVG {}: {}", index, e);
                        None
                    }
                }
            })
            .collect::<Vec<_>>()
            .into_iter()
            .filter_map(|x| x)
            .collect();

        let rendered_pixmaps: Vec<_> = parsed_trees
            .par_iter()
            .map(|(idx, tree)| {
                let pixmap_size = tree.size().to_int_size();

                if pixmap_size.width() == 0 || pixmap_size.height() == 0 {
                    return (*idx, Err("Invalid dimensions".to_string()));
                }

                match tiny_skia::Pixmap::new(pixmap_size.width(), pixmap_size.height()) {
                    Some(mut pixmap) => {
                        resvg::render(tree, tiny_skia::Transform::default(), &mut pixmap.as_mut());
                        (*idx, Ok(pixmap))
                    }
                    None => (*idx, Err("Failed to create pixmap".to_string())),
                }
            })
            .collect();

        let encoded_results: Vec<_> = rendered_pixmaps
            .par_iter()
            .map(|(idx, pixmap_result)| {
                match pixmap_result {
                    Ok(pixmap) => {
                        match pixmap.encode_png() {
                            Ok(png_data) => {
                                let estimated_capacity = (png_data.len() * 4) / 3 + 30;
                                let mut result = String::with_capacity(estimated_capacity);
                                result.push_str("data:image/png;base64,");
                                base64::engine::general_purpose::STANDARD.encode_string(&png_data, &mut result);
                                (*idx, TaskResult { base64_data: result, error: None })
                            }
                            Err(e) => (*idx, TaskResult {
                                base64_data: String::new(),
                                error: Some(format!("PNG encoding error: {}", e))
                            }),
                        }
                    }
                    Err(e) => (*idx, TaskResult {
                        base64_data: String::new(),
                        error: Some(e.clone())
                    }),
                }
            })
            .collect();

        let mut results = vec![TaskResult { base64_data: String::new(), error: Some("Not processed".to_string()) }; svg_data_list.len()];
        for (idx, result) in encoded_results {
            results[*idx] = result;
        }
        results
    }
}