use std::path::PathBuf;

use clap::{Args, Parser, Subcommand, ValueEnum};

#[derive(Debug, Parser)]
#[command(name = "rb", version, about = "Run Rubicon applications")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Debug, Subcommand)]
pub enum Commands {
    Build(BuildArgs),
    Serve(ServeArgs),
}

#[derive(Debug, Args)]
pub struct BuildArgs {
    #[arg(long, value_name = "FILE", default_value = "rubicon.toml")]
    pub config: PathBuf,
    #[arg(long, value_enum, default_value = "rubicon", conflicts_with = "node")]
    pub backend: Backend,
    #[arg(long)]
    pub node: bool,
    #[arg(long)]
    pub project: Option<String>,
    #[arg(long)]
    pub configuration: Option<String>,
    #[arg(long)]
    pub dry_run: bool,
}

#[derive(Debug, Args)]
pub struct ServeArgs {
    #[arg(long, value_name = "FILE", default_value = "rubicon.toml")]
    pub config: PathBuf,

    #[arg(long, value_enum, default_value = "rubicon", conflicts_with = "node")]
    pub backend: Backend,

    #[arg(long)]
    pub node: bool,

    #[arg(long, help = "Show launch configuration without starting the server")]
    pub dry_run: bool,

    #[arg(long, help = "Also stop when standard input closes (for automation)")]
    pub managed: bool,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum Backend {
    Rubicon,
    Node,
}

#[cfg(test)]
mod tests {
    use super::{Backend, Cli, Commands};
    use clap::Parser;

    #[test]
    fn build_and_serve_default_to_local_configuration_and_rubicon() {
        let Commands::Build(build) = Cli::try_parse_from(["rb", "build"]).unwrap().command else {
            panic!("Expected build");
        };
        assert_eq!(build.config.to_str(), Some("rubicon.toml"));
        assert!(matches!(build.backend, Backend::Rubicon));
        assert!(!build.node);
        let Commands::Serve(serve) = Cli::try_parse_from(["rb", "serve"]).unwrap().command else {
            panic!("Expected serve");
        };
        assert_eq!(serve.config.to_str(), Some("rubicon.toml"));
        assert!(matches!(serve.backend, Backend::Rubicon));
    }

    #[test]
    fn node_alias_accepts_defaults_but_rejects_an_explicit_backend() {
        for command in ["build", "serve"] {
            assert!(Cli::try_parse_from(["rb", command, "--node"]).is_ok());
            assert!(Cli::try_parse_from(["rb", command, "--backend", "node"]).is_ok());
            assert!(
                Cli::try_parse_from(["rb", command, "--node", "--backend", "rubicon"]).is_err()
            );
        }
    }
}
