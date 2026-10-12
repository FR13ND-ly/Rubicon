use std::{
    fs,
    net::SocketAddr,
    path::{Component, Path, PathBuf},
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
