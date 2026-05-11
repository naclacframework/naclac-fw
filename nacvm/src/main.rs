mod commands;
mod config;
mod utils;

use clap::{Parser, Subcommand};
use config::Config;

#[derive(Parser)]
#[command(name = "nacvm")]
#[command(version, about = "Naclac Version Manager", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Install a specific version of naclac
    Install {
        version: String,
    },
    /// Use a specific version of naclac globally
    Use {
        version: String,
    },
    /// Uninstall a specific version of naclac
    Uninstall {
        version: String,
    },
    /// List all installed versions
    List,
}

fn main() {
    let cli = Cli::parse();
    let config = Config::init();

    match &cli.command {
        Commands::Install { version } => {
            commands::install::execute(&config, version);
        }
        Commands::Use { version } => {
            commands::use_ver::execute(&config, version);
        }
        Commands::Uninstall { version } => {
            commands::uninstall::execute(&config, version);
        }
        Commands::List => {
            commands::list::execute(&config);
        }
    }
}
