use std::{
    env::{self, VarError},
    net::SocketAddr,
    path::PathBuf,
};

#[derive(Debug, Clone, Copy)]
pub struct RendererConfig {
    pub max_in_flight: usize,
    pub queue_capacity: usize,
}

impl Default for RendererConfig {
    fn default() -> Self {
        Self {
            max_in_flight: 16,
            queue_capacity: 16,
        }
    }
}

impl RendererConfig {
    pub(crate) fn validate(self) -> Result<(), String> {
        if self.max_in_flight == 0 {
            return Err("Renderer max_in_flight must be at least 1.".to_owned());
        }
        if self.queue_capacity == 0 || self.queue_capacity > tokio::sync::Semaphore::MAX_PERMITS {
            return Err("Renderer queue_capacity is outside the supported range.".to_owned());
        }
        Ok(())
    }
}

#[derive(Debug)]
pub struct ServerConfig {
    pub bind: SocketAddr,
    pub origin: String,
    pub server_entry: PathBuf,
    pub browser_dir: PathBuf,
    pub renderer: RendererConfig,
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

        if let Some(value) = read_text("RUBICON_MAX_IN_FLIGHT")? {
            self.renderer.max_in_flight = parse_capacity("RUBICON_MAX_IN_FLIGHT", &value)?;
        }
        if let Some(value) = read_text("RUBICON_QUEUE_CAPACITY")? {
            self.renderer.queue_capacity = parse_capacity("RUBICON_QUEUE_CAPACITY", &value)?;
        }
        self.renderer.validate()?;

        Ok(self)
    }
}

fn parse_capacity(name: &str, value: &str) -> Result<usize, String> {
    value
        .parse()
        .map_err(|_| format!("{name} must be a positive integer."))
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

#[cfg(test)]
mod tests {
    use super::{RendererConfig, parse_capacity};

    #[test]
    fn renderer_rejects_invalid_admission_limits() {
        for config in [
            RendererConfig {
                max_in_flight: 0,
                queue_capacity: 16,
            },
            RendererConfig {
                max_in_flight: 16,
                queue_capacity: 0,
            },
            RendererConfig {
                max_in_flight: 16,
                queue_capacity: usize::MAX,
            },
        ] {
            assert!(config.validate().is_err());
        }
        for max_in_flight in [1, 4, 16] {
            assert!(
                RendererConfig {
                    max_in_flight,
                    queue_capacity: 16
                }
                .validate()
                .is_ok()
            );
        }
        for value in ["", "-1", "1.5", "not-a-number"] {
            assert!(parse_capacity("RUBICON_MAX_IN_FLIGHT", value).is_err());
        }
    }
}
