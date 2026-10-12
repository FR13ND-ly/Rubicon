use std::{path::Path, process::Command};

use crate::{args::Backend, config::ApplicationConfig};

pub fn prepare(
    application: &ApplicationConfig,
    backend: Backend,
    config_path: &Path,
) -> Result<Command, String> {
    let settings = match backend {
        Backend::Rubicon => &application.rubicon,
        Backend::Node => &application.node,
    };

    let absolute_config = std::path::absolute(config_path)
        .map_err(|error| format!("Cannot resolve configuration path: {error}"))?;

    let working_dir = absolute_config
        .parent()
        .ok_or_else(|| "Configuration path has no parent directory.".to_owned())?;

    let mut command = Command::new(&settings.executable);

    command
        .current_dir(working_dir)
        .env("RUBICON_BIND", application.server.bind.to_string())
        .env("RUBICON_ORIGIN", &application.server.origin)
        .env("RUBICON_SERVER_ENTRY", &settings.server_entry)
        .env("RUBICON_BROWSER_DIR", &settings.browser_dir);

    if let Backend::Node = backend {
        command
            .arg(&settings.server_entry)
            .env("HOST", application.server.bind.ip().to_string())
            .env("PORT", application.server.bind.port().to_string());
    }

    Ok(command)
}
