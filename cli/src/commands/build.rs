use {
    crate::{
        cargo_progress,
        commands::{programs, workspace},
        ui,
    },
    colored::Colorize,
    std::{
        fs,
        path::PathBuf,
        process::{Command, Stdio},
    },
};

// The platform-tools versions naclac recommends, newest first. One already
// cached locally is used as-is; with none cached the newest is downloaded by
// `cargo build-sbf`.
const ACCEPTED_PLATFORM_TOOLS: [&str; 2] = ["v1.58", "v1.56"];
const DEFAULT_ARCH: &str = "v3";

fn build_spawn_error(error: &std::io::Error) -> String {
    format!(
        "Cannot run cargo build-sbf: {error}. Install the Solana CLI tools, which provide it: \
         https://solana.com/docs/intro/installation"
    )
}

/// Highest version in `ACCEPTED_PLATFORM_TOOLS` that `cargo-build-sbf` has
/// already downloaded to its cache — i.e. one we can use without triggering
/// a fresh download.
fn cached_accepted_platform_tools_version() -> Option<&'static str> {
    let cache_dir = workspace::home_dir().join(".cache/solana");
    ACCEPTED_PLATFORM_TOOLS
        .iter()
        .find(|v| cache_dir.join(v).join("platform-tools").exists())
        .copied()
}

const ALLOWED_ARCH_VALUES: [&str; 5] = ["v0", "v1", "v2", "v3", "v4"];

/// Resolves the `--tools-version`/`--arch` naclac passes to every
/// `cargo build-sbf` invocation. With no `[build]` section in `Naclac.toml`,
/// naclac uses whichever version in `ACCEPTED_PLATFORM_TOOLS` is already
/// cached locally (the newest of them is downloaded if none is) and
/// `DEFAULT_ARCH`. `[build] platform_tools_version` is a trusted override —
/// any version string is passed through as-is, including ones released after
/// `ACCEPTED_PLATFORM_TOOLS` was last updated. `[build] arch` is instead
/// validated against `ALLOWED_ARCH_VALUES`. A `[build]` value that is not a
/// string is an error.
pub fn resolve_build_toolchain(config: &toml::Value) -> (String, String) {
    let build_section = config.get("build");

    let tools_version = match build_section.and_then(|b| b.get("platform_tools_version")) {
        Some(value) => value.as_str().map(str::to_string).unwrap_or_else(|| {
            ui::error(
                "Naclac.toml [build] platform_tools_version must be a string, e.g. \"v1.58\".",
            )
        }),
        None => cached_accepted_platform_tools_version()
            .unwrap_or(ACCEPTED_PLATFORM_TOOLS[0])
            .to_string(),
    };

    let arch = match build_section.and_then(|b| b.get("arch")) {
        Some(value) => match value.as_str() {
            Some(requested) if ALLOWED_ARCH_VALUES.contains(&requested) => requested.to_string(),
            Some(requested) => ui::error(format!(
                "Naclac.toml [build] arch = \"{}\" is not supported — expected one of {:?}.",
                requested, ALLOWED_ARCH_VALUES
            )),
            None => ui::error(format!(
                "Naclac.toml [build] arch must be a string, one of {:?}.",
                ALLOWED_ARCH_VALUES
            )),
        },
        None => DEFAULT_ARCH.to_string(),
    };

    (tools_version, arch)
}

/// Builds each program in `requested` (every program Naclac.toml lists when
/// empty; see `programs`).
pub fn execute(
    requested: &[String],
    features: Vec<String>,
    show_output: bool,
    extra_args: Vec<String>,
) {
    let build_start = std::time::Instant::now();
    let workspace_root = workspace::root();
    let config = workspace::config(&workspace_root);

    let (tools_version, arch) = resolve_build_toolchain(&config);

    let programs_to_build: Vec<PathBuf> = programs::select(&programs::listed(&config), requested)
        .iter()
        .map(|name| programs::program_dir(&workspace_root, name))
        .collect();

    ui::header(
        "build",
        format!(
            "{} program{}",
            programs_to_build.len(),
            if programs_to_build.len() == 1 {
                ""
            } else {
                "s"
            }
        ),
    );
    ui::kv(
        "tools",
        format!("platform-tools {tools_version} · arch {arch}"),
    );

    crate::commands::generate::ensure_client_stubs(
        &workspace_root,
        &programs::all_dirs(&workspace_root),
    );

    let deploy_dir = naclac_client_gen::deploy_dir(&workspace_root);
    fs::create_dir_all(&deploy_dir)
        .unwrap_or_else(|e| ui::error(format!("Cannot create {}: {e}", deploy_dir.display())));
    let legacy_deploy_dir = naclac_client_gen::resolve_target_dir(&workspace_root)
        .unwrap_or_else(|e| ui::error(e))
        .join("deploy");

    let mut addresses: Vec<String> = Vec::new();
    for program_dir in &programs_to_build {
        let program_name = programs::dir_name(program_dir);
        let keypair_file_name = format!("{}-keypair.json", program_name);
        let keypair_path = deploy_dir.join(&keypair_file_name);

        if !keypair_path.exists() {
            let legacy_keypair_path = legacy_deploy_dir.join(&keypair_file_name);
            let legacy_exists = legacy_keypair_path.try_exists().unwrap_or_else(|e| {
                ui::error(format!(
                    "Cannot check {} for an existing keypair: {}",
                    legacy_keypair_path.display(),
                    e
                ))
            });
            if legacy_exists {
                ui::error(format!(
                    "Keypair for '{}' found at {} but not at {} — move it there, or a new \
                     keypair (and so a new program ID) would be generated.",
                    program_name,
                    legacy_keypair_path.display(),
                    keypair_path.display()
                ));
            }
            ui::warn(format!(
                "Keypair missing for '{}' — generated",
                program_name
            ));
            let keygen = Command::new("solana-keygen")
                .arg("new")
                .arg("--no-bip39-passphrase")
                .arg("-o")
                .arg(&keypair_path)
                .arg("--force")
                .output()
                .unwrap_or_else(|e| ui::error(format!("Failed to run solana-keygen: {}", e)));
            if !keygen.status.success() {
                ui::error(format!(
                    "solana-keygen failed to generate a keypair for '{}': {}",
                    program_name,
                    String::from_utf8_lossy(&keygen.stderr).trim()
                ));
            }
        }

        let address_output = Command::new("solana-keygen")
            .arg("pubkey")
            .arg(&keypair_path)
            .output()
            .unwrap_or_else(|e| ui::error(format!("Failed to run solana-keygen: {}", e)));
        let actual_address = String::from_utf8_lossy(&address_output.stdout)
            .trim()
            .to_string();
        if !address_output.status.success() || actual_address.is_empty() {
            ui::error(format!(
                "Could not read the program address from {}: {}",
                keypair_path.display(),
                String::from_utf8_lossy(&address_output.stderr).trim()
            ));
        }

        let lib_path = program_dir.join("src/lib.rs");
        if lib_path.exists() {
            let mut lib_code = fs::read_to_string(&lib_path)
                .unwrap_or_else(|e| ui::error(format!("Cannot read {}: {e}", lib_path.display())));
            if crate::commands::generate::depends_on_naclac_lang(program_dir)
                && programs::declared_id(&lib_code).is_none()
            {
                ui::error(format!(
                    "'{}' must declare its address with `declare_id!(\"<address>\")` in {}; \
                     without it there is no program address to build for.",
                    program_name,
                    lib_path.display()
                ));
            }
            if let Some(start) = lib_code.find("declare_id!(\"") {
                let addr_start = start + 13;
                if let Some(end_offset) = lib_code[addr_start..].find("\")") {
                    let current_address = &lib_code[addr_start..addr_start + end_offset];
                    if current_address != actual_address {
                        ui::warn(format!("Synced program ID -> lib.rs ({})", program_name));
                        lib_code
                            .replace_range(addr_start..addr_start + end_offset, &actual_address);
                        fs::write(&lib_path, lib_code).unwrap_or_else(|e| {
                            ui::error(format!("Cannot write {}: {e}", lib_path.display()))
                        });
                    }
                }
            }
        }

        let toml_path = workspace_root.join("Naclac.toml");
        let toml_code = fs::read_to_string(&toml_path)
            .unwrap_or_else(|e| ui::error(format!("Cannot read {}: {e}", toml_path.display())));
        if let Some(updated) =
            programs::with_synced_address(&toml_code, &program_name, &actual_address)
        {
            ui::warn(format!(
                "Synced program ID -> Naclac.toml ({})",
                program_name
            ));
            fs::write(&toml_path, updated).unwrap_or_else(|e| {
                ui::error(format!("Cannot write {}: {e}", toml_path.display()))
            });
        }
        addresses.push(actual_address);
    }

    for (build_dir, address) in programs_to_build.iter().zip(&addresses) {
        let prog_name = programs::dir_name(build_dir);
        let prog_name = prog_name.as_str();
        let cargo_toml_path = build_dir.join("Cargo.toml");

        // Shared with `naclac generate idl` — see generate.rs, the single
        // implementation of "what does regenerating a program's IDL mean."
        let prog_features = if crate::commands::generate::depends_on_naclac_lang(build_dir) {
            Some(
                crate::commands::generate::detect_program_features(&workspace_root, build_dir)
                    .unwrap_or_else(|e| ui::error(format!("Cannot build '{}': {}", prog_name, e))),
            )
        } else {
            None
        };
        let mode_label = prog_features
            .as_ref()
            .map_or_else(|| "plain program".to_string(), |f| f.label());

        ui::section(
            prog_name,
            format!("{} · {}", ui::short(address), mode_label),
        );

        let mut cmd = Command::new("cargo");
        cmd.arg("build-sbf")
            .arg("--manifest-path")
            .arg(&cargo_toml_path)
            .arg("--tools-version")
            .arg(&tools_version)
            .arg("--arch")
            .arg(&arch)
            .arg("--sbf-out-dir")
            .arg(&deploy_dir)
            .env("CARGO_TERM_COLOR", "always")
            .current_dir(&workspace_root)
            .stdout(Stdio::inherit());
        if !features.is_empty() {
            cmd.arg("--features").arg(features.join(","));
        }
        cmd.args(&extra_args);

        if show_output {
            // Both streams inherited, untouched — cargo sees a real terminal
            // (when one is actually attached) and draws its own native
            // progress bar, instead of the piped-stderr plain-line mode the
            // spinner path below relies on.
            let build_status = cmd
                .stderr(Stdio::inherit())
                .status()
                .unwrap_or_else(|e| ui::error(build_spawn_error(&e)));
            if !build_status.success() {
                ui::error(format!("Compile failed for '{}'", prog_name));
            }
            ui::success("program compiled");
        } else {
            let mut child = cmd
                .envs(cargo_progress::ENV)
                .stderr(Stdio::piped())
                .spawn()
                .unwrap_or_else(|e| ui::error(build_spawn_error(&e)));
            let stderr = child
                .stderr
                .take()
                .unwrap_or_else(|| ui::error("Cannot read the output of cargo build-sbf."));

            let step = ui::Step::start("Compiling the program");
            let mut combined = String::new();
            let mut compiled = 0u32;
            let mut last_progress_total: Option<u32> = None;
            let mut printed_output_header = false;
            let mut cargo_error_summary: Option<String> = None;
            let read = cargo_progress::read_updates(stderr, |line| {
                if let Some(progress) = cargo_progress::parse_progress(line) {
                    step.set_message(format!(
                        "Compiling the program  {}",
                        cargo_progress::render(&progress)
                    ));
                    last_progress_total = Some(progress.total as u32);
                    return;
                }
                combined.push_str(line);
                combined.push('\n');
                if is_cargo_compiling_line(line) {
                    compiled += 1;
                    return;
                }
                if is_cargo_status_line(line) || line.trim().is_empty() {
                    return;
                }
                // Real rustc diagnostic line — show it.
                if !printed_output_header {
                    step.print_above("── compiler output ──────────────────────".dimmed());
                    printed_output_header = true;
                }
                // cargo's own final tally line ("error: could not compile `X` due to N
                // previous errors") — the authoritative error count, straight from cargo.
                let stripped = strip_ansi(line);
                let trimmed = stripped.trim();
                if trimmed.starts_with("error: could not compile")
                    || trimmed.starts_with("error: aborting due to")
                {
                    cargo_error_summary = Some(trimmed.to_string());
                }
                step.print_above(line);
            });

            if let Err(e) = read {
                step.fail(format!("Cannot read the output of cargo build-sbf: {e}"));
            }

            let build_status = match child.wait() {
                Ok(status) => status,
                Err(e) => step.fail(format!("Cannot wait for cargo build-sbf: {e}")),
            };
            let stack_overflow_detected = combined
                .contains("overflows the maximum allowed frame space")
                || combined.contains("exceeded max offset");

            if !build_status.success() || stack_overflow_detected {
                if stack_overflow_detected {
                    step.fail(format!(
                        "'{}' hit a stack-frame overflow — see 'overflows the maximum allowed frame \
                         space' above for which function to fix.",
                        prog_name
                    ));
                } else {
                    match cargo_error_summary {
                        Some(summary) => {
                            step.fail(format!("Compile failed for '{}' — {}", prog_name, summary))
                        }
                        None => step.fail(format!("Compile failed for '{}'", prog_name)),
                    }
                }
            }
            step.done(match last_progress_total {
                None => "program compiled · up to date".to_string(),
                Some(steps) if compiled == 0 => format!(
                    "program compiled · {steps} step{} built",
                    if steps == 1 { "" } else { "s" }
                ),
                Some(steps) => format!(
                    "program compiled · {compiled} crate{} · {steps} step{} built",
                    if compiled == 1 { "" } else { "s" },
                    if steps == 1 { "" } else { "s" },
                ),
            });
        }

        let Some(prog_features) = prog_features else {
            continue;
        };

        if !crate::commands::generate::can_generate(build_dir, prog_name) {
            continue;
        }

        // A real `idl-build` compile can take well over a minute; with
        // `show_output` its cargo output is inherited straight to the
        // terminal, which would otherwise fight the redrawing spinner for
        // the same line — skip the spinner in that case, same as the
        // `cargo build-sbf` step above.
        let idl_step = if show_output {
            None
        } else {
            Some(ui::Step::start("Generating the IDL"))
        };

        // Shared with `naclac generate idl` — see generate.rs.
        let idl_json_pretty = crate::commands::generate::generate_idl_for_program(
            &workspace_root,
            build_dir,
            prog_name,
            &prog_features,
            show_output,
        );

        let idl_dir = naclac_client_gen::idl_dir(&workspace_root);
        fs::create_dir_all(&idl_dir)
            .unwrap_or_else(|e| ui::error(format!("Cannot create {}: {e}", idl_dir.display())));

        let ts_content = match naclac_client_gen::generate_ts(&idl_json_pretty) {
            Ok(code) => code,
            Err(e) => {
                let msg = format!("Failed to generate TS types for '{}': {}", prog_name, e);
                match idl_step {
                    Some(step) => step.fail(msg),
                    None => ui::error(msg),
                }
            }
        };

        let ts_path = idl_dir.join(format!("{}.ts", prog_name));
        fs::write(&ts_path, ts_content)
            .unwrap_or_else(|e| ui::error(format!("Cannot write {}: {e}", ts_path.display())));
        let idl_written = format!("IDL written · {prog_name}.json · {prog_name}.ts");
        match idl_step {
            Some(step) => step.done(idl_written),
            None => ui::success(idl_written),
        }
        crate::commands::generate::execute_client(&[prog_name.to_string()], None);
    }

    ui::milestone(format!(
        "Built {} program{} · {}",
        programs_to_build.len(),
        if programs_to_build.len() == 1 {
            ""
        } else {
            "s"
        },
        ui::format_duration(build_start.elapsed())
    ));
}

/// Removes ANSI CSI escape sequences (`\x1b[...<letter>`), the only kind
/// cargo emits, so line-content checks on a captured line see the same text
/// a human reads.
pub(crate) fn strip_ansi(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut chars = s.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\u{1b}' && chars.peek() == Some(&'[') {
            chars.next();
            for c2 in chars.by_ref() {
                if c2.is_ascii_alphabetic() {
                    break;
                }
            }
            continue;
        }
        out.push(c);
    }
    out
}

/// True for any of cargo's "Compiling <name> v<ver>" stderr lines, covering
/// both workspace members and registry dependencies. Used alongside
/// `is_cargo_status_line` to count compiled crates separately from the
/// total step count (which also includes build-script runs).
fn is_cargo_compiling_line(raw: &str) -> bool {
    let line = strip_ansi(raw);
    let trimmed = line.trim();
    let Some(rest) = trimmed.strip_prefix("Compiling ") else {
        return false;
    };
    let Some(v_pos) = rest.find(" v") else {
        return false;
    };
    rest[v_pos + 2..].starts_with(|c: char| c.is_ascii_digit())
}

/// True for any cargo informational status line — one whose first
/// whitespace-delimited token is an all-ASCII-alphabetic word that starts
/// with an uppercase letter (e.g. Compiling, Finished, Cleaning, Downloaded,
/// Fetch, Blocking, Fresh, Updating, Locking, Unpacking, Running, …).
/// Rustc diagnostic lines (warning/error/note/help, source spans `-->`,
/// pipe-column `|` lines) all begin with lowercase or punctuation and are
/// never matched by this predicate.
pub(crate) fn is_cargo_status_line(raw: &str) -> bool {
    let line = strip_ansi(raw);
    let trimmed = line.trim();
    let end = trimmed
        .find(|c: char| !c.is_ascii_alphabetic())
        .unwrap_or(trimmed.len());
    if end == 0 {
        return false;
    }
    let word = &trimmed[..end];
    word.starts_with(|c: char| c.is_ascii_uppercase())
        && (end == trimmed.len() || trimmed.as_bytes()[end] == b' ')
}

#[cfg(kani)]
mod kani_proofs {
    use super::*;

    /// Proves `strip_ansi`/`is_cargo_compiling_line`/`is_cargo_status_line`
    /// never panic for any valid UTF-8 input — including adversarial
    /// ANSI/unicode sequences a real cargo subprocess could plausibly emit.
    /// A small symbolic byte buffer validated as UTF-8 (Kani explores every
    /// valid encoding within it) stands in for arbitrary subprocess output.
    #[kani::proof]
    #[kani::unwind(10)]
    fn prove_build_line_parsers_never_panic() {
        let bytes: [u8; 12] = kani::any();
        if let Ok(s) = core::str::from_utf8(&bytes) {
            let _ = strip_ansi(s);
            let _ = is_cargo_compiling_line(s);
            let _ = is_cargo_status_line(s);
        }
    }
}
