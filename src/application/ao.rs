#[derive(Debug,Clone,serde::Serialize,serde::Deserialize)]
pub struct SVGListAO {
    pub svg_list: Vec<String>,
}