use std::{
    path::Path,
    process::{Command, Stdio},
    time::Duration,
};

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
        .stdin(Stdio::piped())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .env("RUBICON_MANAGED", "1")
        .env("RUBICON_BIND", application.server.bind.to_string())
        .env("RUBICON_ORIGIN", &application.server.origin)
        .env("RUBICON_SERVER_ENTRY", &settings.server_entry)
        .env("RUBICON_BROWSER_DIR", &settings.browser_dir);

    match backend {
        Backend::Rubicon => {
            command
                .env("RUBICON_WORKERS", application.renderer.workers.to_string())
                .env(
                    "RUBICON_MAX_IN_FLIGHT",
                    application.renderer.max_in_flight.to_string(),
                )
                .env(
                    "RUBICON_QUEUE_CAPACITY",
                    application.renderer.queue_capacity.to_string(),
                );
        }
        Backend::Node => {
            command
                .arg(&settings.server_entry)
                .env("HOST", application.server.bind.ip().to_string())
                .env("PORT", application.server.bind.port().to_string());
        }
    }

    Ok(command)
}

pub async fn run(command: Command, managed: bool) -> Result<(), String> {
    let shutdown = crate::control::signal(managed)?;
    let program = command.get_program().to_string_lossy().into_owned();

    let mut command = tokio::process::Command::from(command);
    command.kill_on_drop(true);

    let mut child = command
        .spawn()
        .map_err(|error| format!("Cannot start '{program}': {error}"))?;

    let input = match child.stdin.take() {
        Some(input) => input,

        None => {
            child
                .kill()
                .await
                .map_err(|error| format!("Server cleanup failed: {error}"))?;

            return Err("Server control pipe was not created.".to_owned());
        }
    };

    let status = tokio::select! {
        result = child.wait() => {
            result.map_err(|error| format!("Server wait failed: {error}"))?
        }

        signal_result = shutdown => {
            drop(input);

            let status = match tokio::time::timeout(
                Duration::from_secs(10),
                child.wait(),
            )
            .await
            {
                Ok(result) => {
                    result.map_err(|error| format!("Server wait failed: {error}"))?
                }

                Err(_) => {
                    child
                        .kill()
                        .await
                        .map_err(|error| format!("Forced server shutdown failed: {error}"))?;

                    return Err(
                        "Server shutdown exceeded 10 seconds; the process was terminated."
                            .to_owned(),
                    );
                }
            };

            signal_result?;
            status
        }
    };

    if status.success() {
        Ok(())
    } else {
        Err(format!("Server exited unsuccessfully: {status}"))
    }
}

#[cfg(test)]
mod tests {
    use super::prepare;
    use crate::{
        args::Backend,
        config::{ApplicationConfig, BackendSettings, RendererSettings, ServerSettings},
    };
    use std::{collections::BTreeMap, path::Path};

    #[test]
    fn renderer_settings_are_scoped_to_the_rust_child() {
        let backend = || BackendSettings {
            executable: "server".into(),
            server_entry: "server.mjs".into(),
            browser_dir: "browser".into(),
        };
        let application = ApplicationConfig {
            angular: Default::default(),
            server: ServerSettings {
                bind: "127.0.0.1:3100".parse().unwrap(),
                origin: "http://127.0.0.1:3100".to_owned(),
            },
            rubicon: backend(),
            node: backend(),
            renderer: RendererSettings {
                workers: 2,
                max_in_flight: 4,
                queue_capacity: 7,
            },
        };
        for (backend, expected) in [(Backend::Rubicon, true), (Backend::Node, false)] {
            let command = prepare(&application, backend, Path::new("rubicon.toml")).unwrap();
            let variables: BTreeMap<_, _> = command
                .get_envs()
                .map(|(key, value)| {
                    (
                        key.to_string_lossy().into_owned(),
                        value.unwrap().to_string_lossy().into_owned(),
                    )
                })
                .collect();
            assert_eq!(
                variables.get("RUBICON_MAX_IN_FLIGHT").map(String::as_str),
                expected.then_some("4")
            );
            assert_eq!(
                variables.get("RUBICON_QUEUE_CAPACITY").map(String::as_str),
                expected.then_some("7")
            );
            assert_eq!(
                variables.get("RUBICON_WORKERS").map(String::as_str),
                expected.then_some("2")
            );
        }
    }
}
