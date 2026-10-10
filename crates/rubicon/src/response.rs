#[derive(Debug, serde::Deserialize)]
pub struct RenderResponse {
    pub status: u16,
    pub headers: Vec<(String, String)>,
    pub body: String,
}
