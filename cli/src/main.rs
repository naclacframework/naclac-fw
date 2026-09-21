use clap::{Parser, Subcommand};
use solana_address::Address;
use std::str::FromStr;

mod commands;
mod ui;

#[derive(Parser)]
#[command(name = "naclac")]
#[command(version, about = "A hyper-optimized native Solana framework generator", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Initializes a new Naclac Workspace
    Init {
        name: String,
        /// Program mode: pinocchio, standard, or optimized
        #[arg(short, long)]
        mode: Option<String>,
    },
    /// Adds a new program to an existing Naclac workspace
    Add {
        name: String,
        /// Program mode: pinocchio, standard, or optimized
        #[arg(short, long)]
        mode: Option<String>,
    },
    /// Builds the SBF binary and generates the IDL JSON
    Build {
        program_id: Option<String>,
        #[arg(short, long)]
        features: Vec<String>,
        /// Stream cargo's own compilation output (progress bar, per-crate
        /// lines) instead of naclac's summarized spinner.
        #[arg(long)]
        show_output: bool,
    },
    /// Regenerates the IDL and/or client SDK(s) from source. Bare `naclac
    /// generate` does both (IDL, then Rust + TypeScript clients) without
    /// running `cargo build-sbf` — use `idl`/`client` to run just one step.
    Generate {
        program_id: Option<String>,
        #[command(subcommand)]
        action: Option<GenerateAction>,
        /// Stream cargo's own compilation output for the `idl-build`
        /// (real-compilation) IDL path instead of running it silently.
        #[arg(long)]
        show_output: bool,
    },
    /// Deploys all programs in the workspace to the configured network
    Deploy { program_id: Option<String> },
    /// Runs the test suite defined in Naclac.toml
    Test {
        /// Optional specific test file to run (e.g. counter_test.test.ts)
        file: Option<String>,
        /// Optional program name to run tests for (e.g. counter)
        #[arg(short, long)]
        program: Option<String>,
        /// Runs the Rust test suite (default)
        #[arg(short, long)]
        rust: bool,
        /// Runs the Node (TypeScript) test suite
        #[arg(short, long)]
        node: bool,
        /// Builds and deploys programs before running tests
        #[arg(short, long)]
        all: bool,
    },
    /// Fetches and deserializes an on-chain account using the local IDL
    Account { address: String },
    /// Upgrades a deployed Naclac program and securely refunds buffer accounts
    Upgrade {
        program_id: Option<String>,
        #[arg(short, long)]
        filepath: Option<String>,
        #[arg(short, long)]
        buffer: Option<String>,
    },
    /// Manages a program's on-chain IDL metadata (Solana's Program Metadata
    /// Program) — upload/update, lock, close, trim, or change authority
    Idl {
        #[command(subcommand)]
        action: IdlAction,
    },
    /// Verifies the determinism of local codebase against an on-chain program
    Verify { program_id: Option<String> },
    /// Checks the system environment for Naclac dependencies
    Doctor,
    /// Cleans the workspace by safely preserving deploy keys, or totally wiping with --hard
    Clean {
        /// Performs a destructive clean, wiping deploy keys in target/deploy!
        #[arg(long)]
        hard: bool,
    },
    /// Airdrops SOL to the current wallet configured in Naclac.toml
    Airdrop {
        /// Amount of SOL to airdrop (default: 1.0)
        #[arg(default_value_t = 1.0)]
        amount: f64,
    },
    /// Tails the logs for the target cluster (or specific program)
    Logs {
        /// Optional specific program ID to filter logs for
        program_id: Option<String>,
    },
    /// Expands macros in the target program using cargo-expand
    Expand {
        /// Optional specific program name to expand
        program_id: Option<String>,
        #[arg(short, long)]
        features: Vec<String>,
    },
    /// Profiles the compute unit usage of the program in real-time
    Profile {
        /// Optional specific program name to profile
        program: Option<String>,
        /// Print rows in the order instructions actually ran, instead of the
        /// default lowest-to-highest CU order
        #[arg(long)]
        run_order: bool,
    },
    /// Runs `cargo clippy` against each program in the workspace, one by one,
    /// automatically using each program's own correct feature flags (e.g.
    /// pinocchio programs get --no-default-features --features pinocchio)
    Check {
        program_id: Option<String>,
        #[arg(short, long)]
        features: Vec<String>,
    },
}

#[derive(Subcommand)]
pub enum GenerateAction {
    /// Regenerates only the IDL JSON from source — no `cargo build-sbf`.
    Idl,
    /// Regenerates client SDK(s) from the existing IDL JSON. With neither
    /// flag (or both), regenerates both Rust and TypeScript.
    Client {
        /// Regenerate only the Rust client SDK
        #[arg(short, long)]
        rust: bool,
        /// Regenerate only the TypeScript client SDK
        #[arg(short, long, alias = "ts")]
        typescript: bool,
    },
}

#[derive(Subcommand)]
pub enum IdlAction {
    /// Creates the on-chain metadata account if it doesn't exist yet, or
    /// updates it in place if it does
    Upload { program_id: Option<String> },
    /// Permanently locks the metadata account against any further
    /// upload/set-authority/trim/close — irreversible
    SetImmutable { program_id: Option<String> },
    /// Permanently deletes the metadata account, refunding its rent
    Close {
        program_id: Option<String>,
        /// Where the reclaimed rent goes (defaults to your own wallet)
        #[arg(short, long)]
        destination: Option<String>,
    },
    /// Resizes the metadata account down to its minimum rent-exempt size,
    /// refunding the excess lamports
    Trim {
        program_id: Option<String>,
        /// Where the reclaimed rent goes (defaults to your own wallet)
        #[arg(short, long)]
        destination: Option<String>,
    },
    /// Changes or removes who can manage the metadata account
    SetAuthority {
        program_id: Option<String>,
        /// The new authority's pubkey
        #[arg(short, long)]
        new_authority: Option<String>,
        /// Remove the authority entirely instead of setting a new one
        #[arg(long)]
        remove: bool,
    },
    /// Fetches and decompresses a program's on-chain IDL, printing it to
    /// stdout (or writing it to a file with --output)
    Fetch {
        program_id: Option<String>,
        #[arg(short, long)]
        output: Option<String>,
    },
}

use std::fs;
use toml::Value;

fn get_target_programs(cli_arg: Option<&String>) -> Vec<String> {
    let current_dir = std::env::current_dir().unwrap();
    let toml_path = if current_dir.join("Naclac.toml").exists() {
        current_dir.join("Naclac.toml")
    } else if current_dir.join("../../Naclac.toml").exists() {
        current_dir
            .join("../../Naclac.toml")
            .canonicalize()
            .unwrap()
    } else {
        eprintln!("❌ Not a Naclac workspace.");
        std::process::exit(1);
    };

    let content = fs::read_to_string(&toml_path).unwrap_or_default();
    let parsed: Value = toml::from_str(&content).unwrap_or(Value::Table(Default::default()));

    let cluster = parsed
        .get("provider")
        .and_then(|p| p.get("cluster"))
        .and_then(|c| c.as_str())
        .unwrap_or("devnet");

    let programs_table = parsed
        .get("programs")
        .and_then(|p| p.get(cluster))
        .and_then(|c| c.as_table());

    if let Some(arg) = cli_arg {
        // A real program address is used as-is. Anything else is treated as
        // a program *name* and resolved against this workspace's own
        // Naclac.toml — a local convenience only, not a global lookup: a
        // name unknown to this workspace's [programs.<cluster>] table fails
        // clearly rather than silently passing a bogus string downstream.
        if Address::from_str(arg).is_ok() {
            return vec![arg.clone()];
        }
        return match programs_table
            .and_then(|t| t.get(arg))
            .and_then(|v| v.as_str())
        {
            Some(id) => vec![id.to_string()],
            None => {
                eprintln!(
                    "❌ '{}' isn't a valid program address, and no program named '{}' was \
                     found in this workspace's Naclac.toml under [programs.{}].",
                    arg, arg, cluster
                );
                std::process::exit(1);
            }
        };
    }

    let mut programs = Vec::new();
    if let Some(programs_table) = programs_table {
        for val in programs_table.values() {
            if let Some(pid) = val.as_str() {
                programs.push(pid.to_string());
            }
        }
    }

    if programs.is_empty() {
        eprintln!("❌ Error: No program IDs found in Naclac.toml for cluster '{}', and no ID was explicitly provided.", cluster);
        std::process::exit(1);
    }

    programs
}

fn main() {
    let cli = Cli::parse();
    match &cli.command {
        Commands::Init { name, mode } => crate::commands::init::execute(name, mode.as_deref()),
        Commands::Add { name, mode } => commands::add::execute(name, mode.as_deref()),
        Commands::Build {
            program_id,
            features,
            show_output,
        } => commands::build::execute(program_id.as_deref(), features.clone(), *show_output),
        Commands::Generate {
            program_id,
            action,
            show_output,
        } => match action {
            None => commands::generate::execute_all(program_id.as_deref(), *show_output),
            Some(GenerateAction::Idl) => {
                commands::generate::execute_idl(program_id.as_deref(), *show_output)
            }
            Some(GenerateAction::Client { rust, typescript }) => {
                // Neither flag (or both) means "generate everything" — only
                // a single flag set alone narrows to just that one SDK.
                let target = match (*rust, *typescript) {
                    (true, false) => Some(commands::generate::ClientKind::Rust),
                    (false, true) => Some(commands::generate::ClientKind::Typescript),
                    _ => None,
                };
                commands::generate::execute_client(program_id.as_deref(), target)
            }
        },
        Commands::Deploy { program_id } => commands::deploy::execute(program_id.as_deref()),
        Commands::Test {
            file,
            program,
            rust,
            node,
            all,
        } => commands::test::execute(file.as_deref(), program.as_deref(), *rust, *node, *all),
        Commands::Account { address } => commands::account::execute(address),
        Commands::Upgrade {
            program_id,
            filepath,
            buffer,
        } => {
            let pids = get_target_programs(program_id.as_ref());
            for pid in pids {
                commands::upgrade::execute(&pid, filepath.as_deref(), buffer.as_deref());
            }
        }
        Commands::Idl { action } => match action {
            IdlAction::Upload { program_id } => {
                let pids = get_target_programs(program_id.as_ref());
                for pid in pids {
                    commands::idl::execute_upload(&pid);
                }
            }
            IdlAction::SetImmutable { program_id } => {
                let pids = get_target_programs(program_id.as_ref());
                for pid in pids {
                    commands::idl::execute_set_immutable(&pid);
                }
            }
            IdlAction::Close {
                program_id,
                destination,
            } => {
                let pids = get_target_programs(program_id.as_ref());
                for pid in pids {
                    commands::idl::execute_close(&pid, destination.as_deref());
                }
            }
            IdlAction::Trim {
                program_id,
                destination,
            } => {
                let pids = get_target_programs(program_id.as_ref());
                for pid in pids {
                    commands::idl::execute_trim(&pid, destination.as_deref());
                }
            }
            IdlAction::SetAuthority {
                program_id,
                new_authority,
                remove,
            } => {
                let pids = get_target_programs(program_id.as_ref());
                for pid in pids {
                    commands::idl::execute_set_authority(&pid, new_authority.as_deref(), *remove);
                }
            }
            IdlAction::Fetch { program_id, output } => {
                let pids = get_target_programs(program_id.as_ref());
                for pid in pids {
                    commands::idl::execute_fetch(&pid, output.as_deref());
                }
            }
        },
        Commands::Verify { program_id } => {
            let pids = get_target_programs(program_id.as_ref());
            for pid in pids {
                commands::verify::execute(&pid);
            }
        }
        Commands::Doctor => commands::doctor::execute(),
        Commands::Clean { hard } => commands::clean::execute(*hard),
        Commands::Airdrop { amount } => commands::airdrop::execute(*amount),
        Commands::Logs { program_id } => {
            let pid = program_id.clone();
            commands::logs::execute(pid.as_deref());
        }
        Commands::Expand {
            program_id,
            features,
        } => commands::expand::execute(program_id.as_deref(), features.clone()),
        Commands::Profile { program, run_order } => {
            commands::profile::execute(program.as_deref(), *run_order)
        }
        Commands::Check {
            program_id,
            features,
        } => commands::check::execute(program_id.as_deref(), features.clone()),
    }
}
