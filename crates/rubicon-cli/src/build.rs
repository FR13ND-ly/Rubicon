use std::{
    path::Path,
    process::{Command, Stdio},
};

use crate::{
    args::{Backend, BuildArgs},
    config::ApplicationConfig,
};

const BUILD_SCRIPT: &str = r#"
import { createRequire } from 'node:module';
import { join } from 'node:path';
import { pathToFileURL } from 'node:url';
const require = createRequire(join(process.cwd(), 'package.json'));
const { buildAngular } = await import(pathToFileURL(require.resolve('@rubicon/angular/build')).href);
const [backend, project, configuration, outputPath, entry, nodeBootstrap, origin] = process.argv.slice(1);
const controller = new AbortController();
const cancel = () => controller.abort(new Error('Build cancelled'));
process.stdin.once('end', cancel);
process.stdin.resume();
try {
  await buildAngular({ backend, project: project || undefined, configuration,
    outputPath, entry, nodeBootstrap: nodeBootstrap || undefined, origin, signal: controller.signal });
} catch (error) {
  console.error(error.message);
  process.exitCode = 1;
} finally {
  process.stdin.off('end', cancel);
  process.stdin.pause();
}
"#;

pub fn prepare(application: &ApplicationConfig, options: &BuildArgs) -> Result<Command, String> {
    let backend = if options.node {
        Backend::Node
    } else {
        options.backend
    };
    let settings = match backend {
        Backend::Rubicon => &application.rubicon,
        Backend::Node => &application.node,
    };
    let output = settings
        .server_entry
        .parent()
        .and_then(Path::parent)
        .ok_or_else(|| "Cannot determine the Angular output directory".to_owned())?;
    if settings.server_entry != output.join("server/server.mjs")
        || settings.browser_dir != output.join("browser")
    {
        return Err(
            "rb build expects server/server.mjs and browser under the same output directory"
                .to_owned(),
        );
    }
    let config = std::path::absolute(&options.config)
        .map_err(|error| format!("Cannot resolve configuration path: {error}"))?;
    let directory = config
        .parent()
        .ok_or_else(|| "Configuration path has no parent directory".to_owned())?;
    let mut command = Command::new(&application.node.executable);
    command
        .current_dir(directory)
        .stdin(Stdio::piped())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .args(["--input-type=module", "--eval", BUILD_SCRIPT, "--"])
        .arg(match backend {
            Backend::Rubicon => "rubicon",
            Backend::Node => "node",
        })
        .arg(
            options
                .project
                .as_ref()
                .or(application.angular.project.as_ref())
                .map(String::as_str)
                .unwrap_or(""),
        )
        .arg(
            options
                .configuration
                .as_deref()
                .unwrap_or(&application.angular.configuration),
        )
        .arg(output)
        .arg(&application.angular.entry)
        .arg(
            application
                .angular
                .node_bootstrap
                .as_deref()
                .unwrap_or_else(|| Path::new("")),
        )
        .arg(&application.server.origin);
    Ok(command)
}

#[cfg(test)]
mod tests {
    use super::{BUILD_SCRIPT, prepare};
    use crate::{
        args::{Backend, BuildArgs},
        config::{AngularSettings, ApplicationConfig, BackendSettings, ServerSettings},
    };
    use std::path::PathBuf;

    fn application() -> ApplicationConfig {
        let base = std::path::absolute("target/build test țară").unwrap();
        let backend = |name: &str| BackendSettings {
            executable: "node".into(),
            server_entry: base.join(name).join("server/server.mjs"),
            browser_dir: base.join(name).join("browser"),
        };
        ApplicationConfig {
            server: ServerSettings {
                bind: "127.0.0.1:3000".parse().unwrap(),
                origin: "http://localhost:3000".into(),
            },
            rubicon: backend("rust"),
            node: backend("node"),
            renderer: Default::default(),
            angular: AngularSettings {
                project: Some("shop".into()),
                entry: base.join("src/server.rubicon.ts"),
                ..Default::default()
            },
        }
    }

    fn options() -> BuildArgs {
        BuildArgs {
            config: PathBuf::from("rubicon.toml"),
            backend: Backend::Rubicon,
            node: false,
            project: None,
            configuration: None,
            dry_run: false,
        }
    }

    #[test]
    fn values_are_separate_arguments_and_build_does_not_require_rust_executable() {
        let mut application = application();
        application.rubicon.executable = "not-built-yet".into();
        let mut options = options();
        options.project = Some("a project 'quoted' $literal".into());
        options.configuration = Some("development".into());
        let command = prepare(&application, &options).unwrap();
        let arguments: Vec<_> = command
            .get_args()
            .map(|value| value.to_string_lossy().into_owned())
            .collect();
        assert_eq!(command.get_program(), "node");
        assert_eq!(
            &arguments[..4],
            ["--input-type=module", "--eval", BUILD_SCRIPT, "--"]
        );
        assert_eq!(arguments[4], "rubicon");
        assert_eq!(arguments[5], "a project 'quoted' $literal");
        assert_eq!(arguments[6], "development");
        assert!(arguments[7].contains("build test țară"));
        assert_eq!(arguments[8], application.angular.entry.to_string_lossy());
        assert_eq!(arguments[9], "");
        assert_eq!(arguments[10], "http://localhost:3000");
    }

    #[test]
    fn node_alias_selects_node_output_and_optional_fixture_bootstrap() {
        let mut application = application();
        application.angular.node_bootstrap =
            Some(application.angular.entry.with_file_name("catalog.node.ts"));
        let mut options = options();
        options.node = true;
        let command = prepare(&application, &options).unwrap();
        let arguments: Vec<_> = command
            .get_args()
            .map(|value| value.to_string_lossy().into_owned())
            .collect();
        assert_eq!(arguments[4], "node");
        assert_eq!(arguments[5], "shop");
        assert!(arguments[7].ends_with("node"));
        assert!(arguments[9].ends_with("catalog.node.ts"));
    }

    #[test]
    fn mismatched_artifact_layout_is_reported_before_build() {
        let mut application = application();
        application.rubicon.browser_dir = application.rubicon.browser_dir.join("other");
        assert!(
            prepare(&application, &options())
                .unwrap_err()
                .contains("same output directory")
        );
    }
}
