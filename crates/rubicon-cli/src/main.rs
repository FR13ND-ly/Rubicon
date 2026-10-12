mod args;
mod config;

use clap::Parser;

use args::{Backend, Cli, Commands};

fn main() -> Result<(), String> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Serve(options) => {
            let application = config::load(&options.config)?;

            let backend = match options.backend {
                Backend::Rubicon => &application.rubicon,
                Backend::Node => &application.node,
            };

            println!("Backend: {:?}", options.backend);
            println!("Bind: {}", application.server.bind);
            println!("Origin: {}", application.server.origin);
            println!("Executable: {}", backend.executable.display());
            println!("Server entry: {}", backend.server_entry.display());
            println!("Browser directory: {}", backend.browser_dir.display());
        }
    }

    Ok(())
}
