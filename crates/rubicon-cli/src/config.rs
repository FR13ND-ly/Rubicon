use std::{
    fs,
    net::SocketAddr,
    path::{Component, Path, PathBuf},
};

use serde::Deserialize;
use url::Url;

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ApplicationConfig {
    pub server: ServerSettings,
    pub rubicon: BackendSettings,
    pub node: BackendSettings,
    #[serde(default)]
    pub renderer: RendererSettings,
}

#[derive(Debug, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct RendererSettings {
    pub workers: usize,
    pub max_in_flight: usize,
    pub queue_capacity: usize,
}

impl Default for RendererSettings {
    fn default() -> Self {
        Self {
            workers: 1,
            max_in_flight: 16,
            queue_capacity: 16,
        }
    }
}

impl RendererSettings {
    fn validate(&self) -> Result<(), String> {
        if self.workers == 0 {
            return Err("renderer.workers must be at least 1.".to_owned());
        }
        if self.max_in_flight == 0 {
            return Err("renderer.max_in_flight must be at least 1.".to_owned());
        }
        if self.queue_capacity == 0 || self.queue_capacity > tokio::sync::Semaphore::MAX_PERMITS {
            return Err("renderer.queue_capacity is outside the supported range.".to_owned());
        }
        Ok(())
    }
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
    let absolute_path = std::path::absolute(path)
        .map_err(|error| format!("Cannot resolve configuration '{}': {error}", path.display()))?;

    let source = fs::read_to_string(&absolute_path).map_err(|error| {
        format!(
            "Cannot read configuration '{}': {error}",
            absolute_path.display(),
        )
    })?;

    let mut application: ApplicationConfig = toml::from_str(&source).map_err(|error| {
        format!(
            "Invalid configuration '{}': {error}",
            absolute_path.display(),
        )
    })?;

    application.server.origin = validate_origin(&application.server.origin)?;
    application.renderer.validate()?;
    let base_dir = absolute_path
        .parent()
        .ok_or_else(|| "Configuration path has no parent directory.".to_owned())?;

    for backend in [&mut application.rubicon, &mut application.node] {
        backend.server_entry = resolve_path(base_dir, &backend.server_entry)?;
        backend.browser_dir = resolve_path(base_dir, &backend.browser_dir)?;
        backend.executable = resolve_executable(base_dir, &backend.executable)?;
    }

    Ok(application)
}

fn resolve_path(base_dir: &Path, value: &Path) -> Result<PathBuf, String> {
    if value.as_os_str().is_empty() {
        return Err("Configured path cannot be empty.".to_owned());
    }

    if matches!(value.components().next(), Some(Component::Prefix(_))) && !value.is_absolute() {
        return Err(format!(
            "Drive-relative paths are not supported: '{}'",
            value.display(),
        ));
    }

    std::path::absolute(base_dir.join(value))
        .map_err(|error| format!("Cannot resolve path '{}': {error}", value.display()))
}

fn resolve_executable(base_dir: &Path, value: &Path) -> Result<PathBuf, String> {
    let is_bare_name = value
        .file_name()
        .is_some_and(|name| value.as_os_str() == name);

    if is_bare_name {
        return Ok(value.to_path_buf());
    }

    let resolved = resolve_path(base_dir, value)?;

    #[cfg(windows)]
    let resolved = if resolved.extension().is_none() {
        resolved.with_extension("exe")
    } else {
        resolved
    };

    Ok(resolved)
}

fn validate_origin(value: &str) -> Result<String, String> {
    if value
        .chars()
        .any(|character| character.is_whitespace() || character.is_control())
    {
        return Err("server.origin cannot contain whitespace or control characters.".to_owned());
    }

    let (scheme, authority) = value
        .split_once("://")
        .ok_or_else(|| "server.origin must start with http:// or https://.".to_owned())?;

    if !scheme.eq_ignore_ascii_case("http") && !scheme.eq_ignore_ascii_case("https") {
        return Err("server.origin must use HTTP or HTTPS.".to_owned());
    }

    let authority = authority.strip_suffix('/').unwrap_or(authority);

    if authority.is_empty()
        || authority
            .chars()
            .any(|character| matches!(character, '/' | '\\' | '?' | '#' | '@'))
    {
        return Err("server.origin must contain only scheme, host, and optional port.".to_owned());
    }

    let parsed = Url::parse(value).map_err(|error| format!("Invalid server.origin: {error}"))?;

    Ok(parsed.origin().ascii_serialization())
}

#[cfg(test)]
mod tests {
    use super::{ApplicationConfig, RendererSettings};

    const BASE: &str = r#"
        [server]
        bind = "127.0.0.1:3000"
        origin = "http://127.0.0.1:3000"
        [rubicon]
        executable = "server"
        server_entry = "server.mjs"
        browser_dir = "browser"
        [node]
        executable = "node"
        server_entry = "server.mjs"
        browser_dir = "browser"
    "#;

    #[test]
    fn old_config_gets_defaults_and_explicit_renderer_settings_override_them() {
        let defaults: ApplicationConfig = toml::from_str(BASE).unwrap();
        assert_eq!(defaults.renderer.max_in_flight, 16);
        assert_eq!(defaults.renderer.queue_capacity, 16);
        let configured: ApplicationConfig = toml::from_str(&format!(
            "{BASE}\n[renderer]\nmax_in_flight = 1\nqueue_capacity = 3\n",
        ))
        .unwrap();
        assert_eq!(configured.renderer.max_in_flight, 1);
        assert_eq!(configured.renderer.queue_capacity, 3);
        assert!(configured.renderer.validate().is_ok());
    }

    #[test]
    fn renderer_rejects_zero_overflow_and_unknown_fields() {
        for settings in [
            RendererSettings {
                workers: 0,
                max_in_flight: 16,
                queue_capacity: 16,
            },
            RendererSettings {
                workers: 1,
                max_in_flight: 0,
                queue_capacity: 16,
            },
            RendererSettings {
                workers: 1,
                max_in_flight: 16,
                queue_capacity: 0,
            },
            RendererSettings {
                workers: 1,
                max_in_flight: 16,
                queue_capacity: usize::MAX,
            },
        ] {
            assert!(settings.validate().is_err());
        }
        assert!(toml::from_str::<RendererSettings>("max_inflight = 4").is_err());
        assert!(toml::from_str::<RendererSettings>("max_in_flight = -1").is_err());
    }
}
