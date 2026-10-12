use std::{
    fs,
    net::SocketAddr,
    path::{Path, PathBuf},
};

use serde::Deserialize;

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ApplicationConfig {
    pub server: ServerSettings,
    pub rubicon: BackendSettings,
    pub node: BackendSettings,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ServerSettings {
    pub bind: SocketAddr,
    pub origin: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BackendSettings {
    pub executable: PathBuf,
    pub server_entry: PathBuf,
    pub browser_dir: PathBuf,
}

pub fn load(path: &Path) -> Result<ApplicationConfig, String> {
    let source = fs::read_to_string(path)
        .map_err(|error| format!("Cannot read configuration '{}': {error}", path.display(),))?;

    toml::from_str(&source)
        .map_err(|error| format!("Invalid configuration '{}': {error}", path.display(),))
}
