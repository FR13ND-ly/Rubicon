mod args;
mod config;
mod control;
mod launcher;

use clap::Parser;

use args::{Cli, Commands};

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), String> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Serve(options) => {
            let application = config::load(&options.config)?;

            let command = launcher::prepare(&application, options.backend, &options.config)?;

            if !options.dry_run {
                return launcher::run(command, options.managed).await;
            }

            println!("Backend: {:?}", options.backend);
            println!("Program: {}", command.get_program().to_string_lossy());
            println!("Arguments: {:?}", command.get_args().collect::<Vec<_>>(),);

            if let Some(directory) = command.get_current_dir() {
                println!("Directory: {}", directory.display());
            }

            for (name, value) in command.get_envs() {
                if let Some(value) = value {
                    println!("{}={}", name.to_string_lossy(), value.to_string_lossy(),);
                }
            }
        }
    }

    Ok(())
}
