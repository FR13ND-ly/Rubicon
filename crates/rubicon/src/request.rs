#[derive(Debug, serde::Serialize)]
pub struct RenderRequest {
    pub request_id: String,
    pub url: String,
    pub method: String,
    pub headers: Vec<(String, String)>,
}
