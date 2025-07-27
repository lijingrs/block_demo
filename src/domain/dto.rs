#[derive(Clone)]
pub struct TaskResult {
    pub base64_data: String,
    pub(crate) error: Option<String>,
}