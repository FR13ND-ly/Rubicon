mod args;

use clap::Parser;

use args::{Cli, Commands};

fn main() {
    let cli = Cli::parse();

    match cli.command {
        Commands::Serve(options) => {
            println!(
                "Parsed arguments: config={}, backend={:?}",
                options.config.display(),
                options.backend,
            );
        }
    }
}
