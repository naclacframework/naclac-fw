use {
    clap::{Parser, Subcommand},
    solana_address::Address,
    std::str::FromStr,
};

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
        /// Execution runtime: pinocchio or solana
        #[arg(short, long)]
        runtime: Option<String>,
        /// Account data layout: bytemuck, wincode, or borsh (solana runtime only)
        #[arg(short, long)]
        mode: Option<String>,
        /// TypeScript SDK framework: kit or legacy
        #[arg(long)]
        ts: Option<String>,
        /// Node package manager: npm, yarn, pnpm, or bun
        #[arg(short = 'p', long = "pm", visible_alias = "package-manager")]
        pm: Option<String>,
        /// Skip installing Node dependencies
        #[arg(long = "no-install")]
        no_install: bool,
        /// Skip initializing a git repository
        #[arg(long = "no-git")]
        no_git: bool,
        /// Build against a local checkout of the framework instead of the
        /// published packages (recorded as `[framework] path` in Naclac.toml)
        #[arg(long = "framework-path")]
        framework_path: Option<String>,
        /// Create the program with no instructions and no example code,
        /// instead of the counter example
        #[arg(long)]
        empty: bool,
    },
    /// Adds a new program to an existing Naclac workspace
    Add {
        name: String,
        /// Execution runtime: pinocchio or solana
        #[arg(short, long)]
        runtime: Option<String>,
        /// Account data layout: bytemuck, wincode, or borsh (solana runtime only)
        #[arg(short, long)]
        mode: Option<String>,
        /// TypeScript SDK framework: kit or legacy
        #[arg(long)]
        ts: Option<String>,
        /// Create the program with no instructions and no example code,
        /// instead of the counter example
        #[arg(long)]
        empty: bool,
    },
    /// Builds the SBF binary and generates the IDL JSON
    Build {
        program_id: Option<String>,
        /// Program to build (e.g. counter); repeat for several
        #[arg(short = 'p', long = "program")]
        programs: Vec<String>,
        #[arg(short, long)]
        features: Vec<String>,
        /// Stream cargo's own compilation output (progress bar, per-crate
        /// lines) instead of naclac's summarized spinner.
        #[arg(long)]
        show_output: bool,
        /// Extra arguments forwarded as-is to the underlying `cargo
        /// build-sbf` invocation, after everything else naclac passes it —
        /// e.g. `naclac build counter_borsh -- --dump` for a disassembly/size
        /// report. Not validated by naclac; any `cargo build-sbf` flag works.
        #[arg(last = true)]
        extra_args: Vec<String>,
    },
    /// Regenerates the IDL and/or client SDK(s) from source. Bare `naclac
    /// generate` does both (IDL, then Rust + TypeScript clients) without
    /// running `cargo build-sbf` — use `idl`/`client` to run just one step.
    Generate {
        program_id: Option<String>,
        /// Program to generate for (e.g. counter); repeat for several
        #[arg(short = 'p', long = "program")]
        programs: Vec<String>,
        #[command(subcommand)]
        action: Option<GenerateAction>,
        /// Stream cargo's own compilation output for the `idl-build`
        /// compile that generates the IDL instead of naclac's summarized
        /// spinner. Accepted before or after `idl` / `client`.
        #[arg(long, global = true)]
        show_output: bool,
    },
    /// Deploys the programs of `[programs.<cluster>]` to the cluster named by
    /// `[provider] cluster`
    Deploy {
        program_id: Option<String>,
        /// Program to deploy (e.g. counter); repeat for several
        #[arg(short = 'p', long = "program")]
        programs: Vec<String>,
        /// Deploy only; do not upload or update the program's IDL
        #[arg(long)]
        no_idl: bool,
        /// Write each IDL even when the account already holds exactly it
        #[arg(long, conflicts_with = "no_idl")]
        force_idl: bool,
        /// Show the detail the summary lines leave out (the deploy tool's own
        /// output)
        #[arg(long)]
        show_output: bool,
    },
    /// Runs the test suite defined in Naclac.toml
    Test {
        /// Optional specific test file to run (e.g. counter_test.test.ts)
        file: Option<String>,
        /// Program to run tests for (e.g. counter); repeat for several
        #[arg(short = 'p', long = "program")]
        programs: Vec<String>,
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
    /// Upgrades deployed Naclac programs and updates their IDLs
    Upgrade {
        program_id: Option<String>,
        /// Program to upgrade (a name from `[programs.<cluster>]` or an
        /// address); repeat for several
        #[arg(short = 'p', long = "program")]
        programs: Vec<String>,
        /// Upgrade from this program file instead of the compiled `.so`
        #[arg(short, long)]
        filepath: Option<String>,
        /// Upgrade from this already written buffer account
        #[arg(short, long)]
        buffer: Option<String>,
        /// Upgrade only; do not update the programs' IDLs
        #[arg(long)]
        no_idl: bool,
        /// Write each IDL even when the account already holds exactly it
        #[arg(long, conflicts_with = "no_idl")]
        force_idl: bool,
        /// Show the detail the summary lines leave out (the upgrade tool's
        /// own output)
        #[arg(long)]
        show_output: bool,
    },
    /// Manages a program's on-chain IDL metadata (Solana's Program Metadata
    /// Program) — upload/update, lock, close, trim, or change authority
    Idl {
        #[command(subcommand)]
        action: IdlAction,
        /// List every transaction with its signature instead of one summary
        /// line. Accepted before or after the action.
        #[arg(long, global = true)]
        show_output: bool,
    },
    /// Verifies the determinism of local codebase against an on-chain program
    Verify {
        program_id: Option<String>,
        /// Program to verify (a name from `[programs.<cluster>]` or an
        /// address); repeat for several
        #[arg(short = 'p', long = "program")]
        programs: Vec<String>,
    },
    /// Checks the system environment for Naclac dependencies
    Doctor,
    /// Removes the build output of each program (dependencies stay cached);
    /// `--hard` removes cargo's whole target directory instead. artifacts/
    /// (including deploy keys) is never touched
    Clean {
        program_id: Option<String>,
        /// Program to clean (e.g. counter); repeat for several
        #[arg(short = 'p', long = "program", conflicts_with = "hard")]
        programs: Vec<String>,
        /// Run a single `cargo clean` for the whole target directory,
        /// dependencies included
        #[arg(long, conflicts_with_all = ["program_id", "programs"])]
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
        /// Optional specific program (a name from `[programs.<cluster>]` or
        /// an address) to filter logs for
        program_id: Option<String>,
        /// Same as the positional program; at most one program can be followed
        #[arg(short = 'p', long = "program")]
        programs: Vec<String>,
    },
    /// Expands macros in the target program using cargo-expand
    Expand {
        /// Optional specific program name to expand
        program_id: Option<String>,
        /// Program to expand (e.g. counter)
        #[arg(short = 'p', long = "program")]
        programs: Vec<String>,
        #[arg(short, long)]
        features: Vec<String>,
    },
    /// Profiles the compute unit usage of the program in real-time
    Profile {
        /// Optional specific program name to profile
        program: Option<String>,
        /// Program to profile (e.g. counter); repeat for several
        #[arg(short = 'p', long = "program")]
        programs: Vec<String>,
        /// Print rows in the order instructions actually ran, instead of the
        /// default lowest-to-highest CU order
        #[arg(long)]
        run_order: bool,
    },
    /// Runs `cargo clippy` against each program in the workspace, one by one,
    /// with each program's own default features.
    /// With --test, lints each program's tests (unit and integration) instead
    /// of the program; run `naclac build` first so the generated client exists
    Check {
        program_id: Option<String>,
        /// Program to check (e.g. counter); repeat for several
        #[arg(short = 'p', long = "program")]
        programs: Vec<String>,
        #[arg(short, long)]
        features: Vec<String>,
        /// Lint the tests instead of the program
        #[arg(long)]
        test: bool,
        /// Fail on any clippy warning, not only on errors
        #[arg(long)]
        deny_warnings: bool,
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
    Upload {
        program_id: Option<String>,
        /// Program (a name from `[programs.<cluster>]` or an address); repeat
        /// for several
        #[arg(short = 'p', long = "program")]
        programs: Vec<String>,
        /// Write the IDL even when the account already holds exactly it
        #[arg(long)]
        force: bool,
    },
    /// Permanently locks the metadata account against any further
    /// upload/set-authority/trim/close — irreversible
    SetImmutable {
        program_id: Option<String>,
        /// Program (a name from `[programs.<cluster>]` or an address); repeat
        /// for several
        #[arg(short = 'p', long = "program")]
        programs: Vec<String>,
        /// Skip the confirmation prompt
        #[arg(long)]
        yes: bool,
    },
    /// Permanently deletes the metadata account, refunding its rent
    Close {
        program_id: Option<String>,
        /// Program (a name from `[programs.<cluster>]` or an address); repeat
        /// for several
        #[arg(short = 'p', long = "program")]
        programs: Vec<String>,
        /// Where the reclaimed rent goes (defaults to your own wallet)
        #[arg(short, long)]
        destination: Option<String>,
        /// Skip the confirmation prompt
        #[arg(long)]
        yes: bool,
    },
    /// Resizes the metadata account down to its minimum rent-exempt size,
    /// refunding the excess lamports
    Trim {
        program_id: Option<String>,
        /// Program (a name from `[programs.<cluster>]` or an address); repeat
        /// for several
        #[arg(short = 'p', long = "program")]
        programs: Vec<String>,
        /// Where the reclaimed rent goes (defaults to your own wallet)
        #[arg(short, long)]
        destination: Option<String>,
    },
    /// Changes or removes who can manage the metadata account
    SetAuthority {
        program_id: Option<String>,
        /// Program (a name from `[programs.<cluster>]` or an address); repeat
        /// for several
        #[arg(short = 'p', long = "program")]
        programs: Vec<String>,
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
        /// Program (a name from `[programs.<cluster>]` or an address); repeat
        /// for several
        #[arg(short = 'p', long = "program")]
        programs: Vec<String>,
        /// File name to write, without `.json` (default: the program's name)
        #[arg(long)]
        name: Option<String>,
        /// Existing folder to write into (default: artifacts/onchain-idl)
        #[arg(long)]
        dir: Option<String>,
        /// Print the IDL instead of writing a file
        #[arg(long, conflicts_with_all = ["name", "dir"])]
        stdout: bool,
        /// Keep the IDL in its on-chain form instead of restoring the local
        /// form (written to `<name>.onchain.json` unless --name is given)
        #[arg(long)]
        raw: bool,
    },
}

/// The programs named on the command line: the positional name, then every
/// `-p` value.
fn requested_programs(positional: &Option<String>, flagged: &[String]) -> Vec<String> {
    positional.iter().chain(flagged).cloned().collect()
}

/// A program a cluster command works on: its name in `[programs.<cluster>]`
/// when it is listed there, and its address.
struct TargetProgram {
    name: Option<String>,
    address: String,
}

/// The programs a cluster command works on, from `[provider] cluster`'s own
/// `[programs.<cluster>]` table (which must exist): an address in `requested`
/// is used as given, a name is looked up in the table, and every program of
/// the table is used when nothing was requested.
fn target_programs(requested: &[String]) -> Vec<TargetProgram> {
    let workspace_root = commands::workspace::root();
    let config = commands::workspace::config(&workspace_root);
    let cluster = commands::workspace::cluster(&config);
    let table = commands::workspace::on_cluster(&config);

    if requested.is_empty() {
        return table
            .into_iter()
            .map(|(name, address)| TargetProgram {
                name: Some(name),
                address,
            })
            .collect();
    }

    requested
        .iter()
        .map(|arg| {
            if Address::from_str(arg).is_ok() {
                let name = table
                    .iter()
                    .find(|(_, address)| address == arg)
                    .map(|(name, _)| name.clone());
                return TargetProgram {
                    name,
                    address: arg.clone(),
                };
            }
            match table.iter().find(|(name, _)| name == arg) {
                Some((name, address)) => TargetProgram {
                    name: Some(name.clone()),
                    address: address.clone(),
                },
                None => ui::error(format!(
                    "`{arg}` is not a valid program address, and no program named `{arg}` is \
                     listed in `[programs.{cluster}]` of Naclac.toml."
                )),
            }
        })
        .collect()
}

/// The addresses of `target_programs`.
fn target_addresses(requested: &[String]) -> Vec<String> {
    target_programs(requested)
        .into_iter()
        .map(|program| program.address)
        .collect()
}

/// Runs `action` on every target, printing each failure as `<label>: <error>`
/// and carrying on with the rest; exits with status 1 if any target failed.
fn run_each<T>(
    targets: Vec<T>,
    label: impl Fn(&T) -> &str,
    mut action: impl FnMut(&T) -> Result<(), String>,
) {
    let mut failed = 0usize;
    for target in &targets {
        if let Err(message) = action(target) {
            ui::error_line(format!("{}: {message}", label(target)));
            failed += 1;
        }
    }
    if failed > 0 {
        ui::error(format!("{failed} of {} failed.", targets.len()));
    }
}

fn main() {
    let cli = Cli::parse();
    if let Commands::Idl { show_output, .. }
    | Commands::Deploy { show_output, .. }
    | Commands::Upgrade { show_output, .. } = &cli.command
    {
        ui::set_show_output(*show_output);
    }
    match &cli.command {
        Commands::Init {
            name,
            runtime,
            mode,
            ts,
            pm,
            no_install,
            no_git,
            framework_path,
            empty,
        } => crate::commands::init::execute(crate::commands::init::InitOptions {
            name,
            runtime: runtime.as_deref(),
            mode: mode.as_deref(),
            ts: ts.as_deref(),
            pm: pm.as_deref(),
            no_install: *no_install,
            no_git: *no_git,
            framework_path: framework_path.as_deref(),
            empty: *empty,
        }),
        Commands::Add {
            name,
            runtime,
            mode,
            ts,
            empty,
        } => commands::add::execute(
            name,
            runtime.as_deref(),
            mode.as_deref(),
            ts.as_deref(),
            *empty,
        ),
        Commands::Build {
            program_id,
            programs,
            features,
            show_output,
            extra_args,
        } => commands::build::execute(
            &requested_programs(program_id, programs),
            features.clone(),
            *show_output,
            extra_args.clone(),
        ),
        Commands::Generate {
            program_id,
            programs,
            action,
            show_output,
        } => {
            let requested = requested_programs(program_id, programs);
            match action {
                None => commands::generate::execute_all(&requested, *show_output),
                Some(GenerateAction::Idl) => {
                    commands::generate::execute_idl(&requested, *show_output)
                }
                Some(GenerateAction::Client { rust, typescript }) => {
                    // Neither flag (or both) means "generate everything" — only
                    // a single flag set alone narrows to just that one SDK.
                    let target = match (*rust, *typescript) {
                        (true, false) => Some(commands::generate::ClientKind::Rust),
                        (false, true) => Some(commands::generate::ClientKind::Typescript),
                        _ => None,
                    };
                    commands::generate::execute_client(&requested, target)
                }
            }
        }
        Commands::Deploy {
            program_id,
            programs,
            no_idl,
            force_idl,
            ..
        } => commands::deploy::execute(
            &requested_programs(program_id, programs),
            *no_idl,
            *force_idl,
        ),
        Commands::Test {
            file,
            programs,
            rust,
            node,
            all,
        } => commands::test::execute(file.as_deref(), programs, *rust, *node, *all),
        Commands::Account { address } => commands::account::execute(address),
        Commands::Upgrade {
            program_id,
            programs,
            filepath,
            buffer,
            no_idl,
            force_idl,
            ..
        } => {
            let targets: Vec<(String, Option<String>)> =
                target_programs(&requested_programs(program_id, programs))
                    .into_iter()
                    .map(|program| (program.address, program.name))
                    .collect();
            commands::upgrade::execute(
                &targets,
                filepath.as_deref(),
                buffer.as_deref(),
                *no_idl,
                *force_idl,
            );
        }
        Commands::Idl { action, .. } => match action {
            IdlAction::Upload {
                program_id,
                programs,
                force,
            } => run_each(
                target_programs(&requested_programs(program_id, programs)),
                |program| program.address.as_str(),
                |program| {
                    let heading = program
                        .name
                        .clone()
                        .unwrap_or_else(|| ui::short(&program.address));
                    ui::section(heading, ui::short(&program.address));
                    let outcome = commands::idl::execute_upload(&program.address, *force);
                    ui::end_section();
                    outcome
                },
            ),
            IdlAction::SetImmutable {
                program_id,
                programs,
                yes,
            } => run_each(
                target_addresses(&requested_programs(program_id, programs)),
                |address| address.as_str(),
                |address| commands::idl::execute_set_immutable(address, *yes),
            ),
            IdlAction::Close {
                program_id,
                programs,
                destination,
                yes,
            } => run_each(
                target_addresses(&requested_programs(program_id, programs)),
                |address| address.as_str(),
                |address| commands::idl::execute_close(address, destination.as_deref(), *yes),
            ),
            IdlAction::Trim {
                program_id,
                programs,
                destination,
            } => run_each(
                target_addresses(&requested_programs(program_id, programs)),
                |address| address.as_str(),
                |address| commands::idl::execute_trim(address, destination.as_deref()),
            ),
            IdlAction::SetAuthority {
                program_id,
                programs,
                new_authority,
                remove,
            } => run_each(
                target_addresses(&requested_programs(program_id, programs)),
                |address| address.as_str(),
                |address| {
                    commands::idl::execute_set_authority(address, new_authority.as_deref(), *remove)
                },
            ),
            IdlAction::Fetch {
                program_id,
                programs,
                name,
                dir,
                stdout,
                raw,
            } => run_each(
                target_programs(&requested_programs(program_id, programs)),
                |program| program.address.as_str(),
                |program| {
                    commands::idl::execute_fetch(
                        &program.address,
                        program.name.as_deref(),
                        name.as_deref(),
                        dir.as_deref(),
                        *stdout,
                        *raw,
                    )
                },
            ),
        },
        Commands::Verify {
            program_id,
            programs,
        } => {
            for program in target_programs(&requested_programs(program_id, programs)) {
                commands::verify::execute(&program.address, program.name.as_deref());
            }
        }
        Commands::Doctor => commands::doctor::execute(),
        Commands::Clean {
            program_id,
            programs,
            hard,
        } => commands::clean::execute(&requested_programs(program_id, programs), *hard),
        Commands::Airdrop { amount } => commands::airdrop::execute(*amount),
        Commands::Logs {
            program_id,
            programs,
        } => {
            let requested = requested_programs(program_id, programs);
            match requested.as_slice() {
                [] => commands::logs::execute(None),
                [_] => {
                    let addresses = target_addresses(&requested);
                    commands::logs::execute(addresses.first().map(String::as_str));
                }
                _ => ui::error(
                    "`naclac logs` follows one program, or every program when none is given.",
                ),
            }
        }
        Commands::Expand {
            program_id,
            programs,
            features,
        } => commands::expand::execute(&requested_programs(program_id, programs), features.clone()),
        Commands::Profile {
            program,
            programs,
            run_order,
        } => commands::profile::execute(&requested_programs(program, programs), *run_order),
        Commands::Check {
            program_id,
            programs,
            features,
            test,
            deny_warnings,
        } => commands::check::execute(
            &requested_programs(program_id, programs),
            features.clone(),
            *test,
            *deny_warnings,
        ),
    }
}
