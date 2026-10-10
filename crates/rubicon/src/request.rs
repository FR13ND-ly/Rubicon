#[derive(Debug, serde::Serialize)]
pub struct RenderRequest {
    pub url: String,
    pub method: String,
}
