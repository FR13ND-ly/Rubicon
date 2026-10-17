use std::process::{Command, Stdio};

use crate::args::IntegrateArgs;

const SCRIPT: &str = r#"
import { createRequire } from 'node:module';
import { join } from 'node:path';
import { pathToFileURL } from 'node:url';
const require = createRequire(join(process.cwd(), 'package.json'));
const [backendPath, project, bin, dryRun] = process.argv.slice(1);
const controller = new AbortController();
const cancel = () => controller.abort(new Error('Integration cancelled'));
process.stdin.once('end', cancel);
process.stdin.resume();
try {
  let entry;
  try { entry = require.resolve('@rubicon/angular/integrate'); }
  catch { throw new Error('Install the current @rubicon/angular SDK in the Angular workspace before running rb integrate.'); }
  const { integrateAngular } = await import(pathToFileURL(entry).href);
  await integrateAngular({ backendPath, project: project || undefined, bin: bin || undefined,
    dryRun: dryRun === 'true', signal: controller.signal });
} catch (error) {
  console.error(error.message);
  process.exitCode = 1;
} finally {
  process.stdin.off('end', cancel);
  process.stdin.pause();
}
"#;

pub fn prepare(options: &IntegrateArgs) -> Result<Command, String> {
    let directory = std::env::current_dir()
        .map_err(|error| format!("Cannot determine the Angular working directory: {error}"))?;
    let mut command = Command::new("node");
    command
        .current_dir(directory)
        .stdin(Stdio::piped())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .args(["--input-type=module", "--eval", SCRIPT, "--"])
        .arg(&options.backend_path)
        .arg(options.project.as_deref().unwrap_or(""))
        .arg(options.bin.as_deref().unwrap_or(""))
        .arg(options.dry_run.to_string());
    Ok(command)
}

#[cfg(test)]
mod tests {
    use super::prepare;
    use crate::{
        args::{Cli, Commands},
        integrate::SCRIPT,
    };
    use clap::Parser;

    #[test]
    fn integration_requires_backend_and_passes_values_without_a_shell() {
        assert!(Cli::try_parse_from(["rb", "integrate"]).is_err());
        let Commands::Integrate(options) = Cli::try_parse_from([
            "rb",
            "integrate",
            "--backend-path",
            "../backend țară '$literal'",
            "--project",
            "shop",
            "--bin",
            "api-server",
            "--dry-run",
        ])
        .unwrap()
        .command
        else {
            panic!("Expected integrate");
        };
        let command = prepare(&options).unwrap();
        let arguments: Vec<_> = command
            .get_args()
            .map(|value| value.to_str().unwrap())
            .collect();
        assert_eq!(command.get_program(), "node");
        assert_eq!(
            arguments,
            [
                "--input-type=module",
                "--eval",
                SCRIPT,
                "--",
                "../backend țară '$literal'",
                "shop",
                "api-server",
                "true"
            ]
        );
    }
}
