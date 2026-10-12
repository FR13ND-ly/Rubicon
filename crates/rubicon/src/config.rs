use std::{
    env::{self, VarError},
    net::SocketAddr,
    path::PathBuf,
};

#[derive(Debug)]
pub struct ServerConfig {
    pub bind: SocketAddr,
    pub origin: String,
    pub server_entry: PathBuf,
    pub browser_dir: PathBuf,
}

impl ServerConfig {
    pub fn with_environment_overrides(mut self) -> Result<Self, String> {
        if let Some(value) = read_text("RUBICON_BIND")? {
            self.bind = value
                .parse::<SocketAddr>()
                .map_err(|error| format!("Invalid RUBICON_BIND: {error}"))?;
        }

        if let Some(value) = read_text("RUBICON_ORIGIN")? {
            self.origin = value;
        }

        if let Some(value) = read_path("RUBICON_SERVER_ENTRY")? {
            self.server_entry = value;
        }

        if let Some(value) = read_path("RUBICON_BROWSER_DIR")? {
            self.browser_dir = value;
        }

        Ok(self)
    }
}

fn read_text(name: &str) -> Result<Option<String>, String> {
    match env::var(name) {
        Ok(value) if value.is_empty() => Err(format!("{name} cannot be empty.")),
        Ok(value) => Ok(Some(value)),
        Err(VarError::NotPresent) => Ok(None),
        Err(VarError::NotUnicode(_)) => Err(format!("{name} must contain valid Unicode.")),
    }
}

fn read_path(name: &str) -> Result<Option<PathBuf>, String> {
    match env::var_os(name) {
        Some(value) if value.is_empty() => Err(format!("{name} cannot be empty.")),
        Some(value) => Ok(Some(PathBuf::from(value))),
        None => Ok(None),
    }
}
