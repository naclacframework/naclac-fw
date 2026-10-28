use {
    crate::{
        commands::{programs, workspace},
        ui,
    },
    colored::Colorize,
    std::{
        fs,
        io::{BufRead, BufReader},
        path::PathBuf,
        process::{Command, Stdio},
    },
};

// platform-tools versions naclac will use as-is if already cached locally,
// newest first.
const ACCEPTED_PLATFORM_TOOLS: [&str; 2] = ["v1.58", "v1.56"];
// Downloaded automatically (via cargo-build-sbf's own fetch-by-version
// support) only when none of `ACCEPTED_PLATFORM_TOOLS` are already cached.
const DEFAULT_PLATFORM_TOOLS_FALLBACK: &str = "v1.56";
const DEFAULT_ARCH: &str = "v3";

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
/// cached locally (or downloads `DEFAULT_PLATFORM_TOOLS_FALLBACK` if none
/// are) and `DEFAULT_ARCH`. `[build] platform_tools_version` is a trusted
/// override — any version string is passed through as-is, including ones
/// released after `ACCEPTED_PLATFORM_TOOLS` was last updated. `[build] arch`
/// is instead validated against `ALLOWED_ARCH_VALUES`.
pub fn resolve_build_toolchain(config: &toml::Value) -> (String, String) {
    let build_section = config.get("build");

    let tools_version = build_section
        .and_then(|b| b.get("platform_tools_version"))
        .and_then(|v| v.as_str())
        .map(str::to_string)
        .or_else(|| cached_accepted_platform_tools_version().map(str::to_string))
        .unwrap_or_else(|| DEFAULT_PLATFORM_TOOLS_FALLBACK.to_string());

    let arch = match build_section
        .and_then(|b| b.get("arch"))
        .and_then(|v| v.as_str())
    {
        Some(requested) if ALLOWED_ARCH_VALUES.contains(&requested) => requested.to_string(),
        Some(requested) => ui::error(format!(
            "Naclac.toml [build] arch = \"{}\" is not supported — expected one of {:?}.",
            requested, ALLOWED_ARCH_VALUES
        )),
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
    ui::info(format!(
        "Using platform-tools {} (arch {})",
        tools_version, arch
    ));

    let programs_to_build: Vec<PathBuf> = programs::select(&programs::listed(&config), requested)
        .iter()
        .map(|name| programs::program_dir(&workspace_root, name))
        .collect();

    crate::commands::generate::ensure_client_stubs(
        &workspace_root,
        &programs::all_dirs(&workspace_root),
    );

    let deploy_dir = naclac_client_gen::deploy_dir(&workspace_root);
    fs::create_dir_all(&deploy_dir).unwrap();
    let legacy_deploy_dir = naclac_client_gen::resolve_target_dir(&workspace_root)
        .unwrap_or_else(|e| ui::error(e))
        .join("deploy");

    for program_dir in &programs_to_build {
        let program_name = program_dir
            .file_name()
            .unwrap()
            .to_str()
            .unwrap()
            .to_string();
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
            let mut lib_code = fs::read_to_string(&lib_path).unwrap();
            if programs::declared_id(&lib_code).is_none() {
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
                        fs::write(&lib_path, lib_code).unwrap();
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
    }

    for build_dir in &programs_to_build {
        let prog_name = build_dir.file_name().unwrap().to_str().unwrap();
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

        let mut cmd = Command::new("cargo");
        cmd.arg("build-sbf")
            .arg("--manifest-path")
            .arg(cargo_toml_path.to_str().unwrap())
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
                .expect("Failed to execute cargo build-sbf");
            if !build_status.success() {
                ui::error(format!("Compile failed for '{}'", prog_name));
            }
            ui::success(format!("Compiled {}", prog_name));
        } else {
            let mut child = cmd
                .stderr(Stdio::piped())
                .spawn()
                .expect("Failed to execute cargo build-sbf");
            let stderr = child
                .stderr
                .take()
                .expect("Failed to capture cargo build-sbf stderr");

            // Piped (not a pseudo-terminal), so cargo skips its own cursor-redrawn
            // progress bar and just prints one plain "Compiling <crate>" line per
            // crate on stderr as they start — parsed below to drive our own
            // single-line spinner instead, since forwarding cargo's raw redraw
            // bytes while dropping lines from the stream desyncs its cursor math
            // (it moves the cursor up assuming every line it emitted is still on
            // screen). Pass `--show-output` to skip this and see cargo's own
            // output directly.
            let step = ui::Step::start(format!("Compiling {} ({})...", prog_name, mode_label));
            let mut combined = String::new();
            let mut compiled = 0u32;
            let mut printed_output_header = false;
            let mut cargo_error_summary: Option<String> = None;
            for line in BufReader::new(stderr).lines() {
                let Ok(line) = line else { break };
                combined.push_str(&line);
                combined.push('\n');
                if let Some(crate_name) = cargo_compiling_crate_name(&line) {
                    compiled += 1;
                    step.set_message(format!(
                        "Compiling {} ({})... {} crate{} built ({})",
                        prog_name,
                        mode_label,
                        compiled,
                        if compiled == 1 { "" } else { "s" },
                        crate_name
                    ));
                } else if !line.trim().is_empty() && !is_cargo_finished_line(&line) {
                    if !printed_output_header {
                        step.print_above("── compiler output ──────────────────────".dimmed());
                        printed_output_header = true;
                    }
                    // cargo's own final tally line ("error: could not compile `X` ...
                    // due to N previous errors" / "error: aborting due to N previous
                    // errors") — the authoritative error count, straight from cargo
                    // itself rather than naclac re-deriving one by pattern-matching
                    // individual diagnostics.
                    let stripped = strip_ansi(&line);
                    let trimmed = stripped.trim();
                    if trimmed.starts_with("error: could not compile")
                        || trimmed.starts_with("error: aborting due to")
                    {
                        cargo_error_summary = Some(trimmed.to_string());
                    }
                    step.print_above(line);
                }
            }

            let build_status = child.wait().expect("Failed to wait on cargo build-sbf");
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
            step.done(format!(
                "Compiled {} ({} crates built)",
                prog_name, compiled
            ));
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
            Some(ui::Step::start(format!(
                "Generating IDL & TS types ({})...",
                prog_name
            )))
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
        fs::create_dir_all(&idl_dir).unwrap();

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
        fs::write(&ts_path, ts_content).unwrap();
        match idl_step {
            Some(step) => step.done(format!("IDL + TS types written ({})", prog_name)),
            None => ui::success(format!("IDL + TS types written ({})", prog_name)),
        }
        crate::commands::generate::execute_client(&[prog_name.to_string()], None);
    }

    ui::milestone(format!(
        "Build complete — {} program{}, {}",
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
fn strip_ansi(s: &str) -> String {
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

/// Extracts the crate name from one of cargo's own per-crate
/// "Compiling <name> v<version> (<path>)" stderr lines, or `None` for any
/// other line (warnings, errors, the final "Finished" line, etc.).
fn cargo_compiling_crate_name(raw: &str) -> Option<String> {
    let line = strip_ansi(raw);
    let trimmed = line.trim();
    let rest = trimmed.strip_prefix("Compiling ")?;
    let v_pos = rest.find(" v")?;
    let after_v = &rest[v_pos + 2..];
    if after_v.starts_with(|c: char| c.is_ascii_digit())
        && after_v.contains('(')
        && trimmed.ends_with(')')
    {
        Some(rest[..v_pos].to_string())
    } else {
        None
    }
}

/// True for cargo's own final "Finished `<profile>` profile [...] target(s)
/// in <N>s" line — dropped because naclac's own timed `✓ <elapsed> Compiled
/// ...` result line (see [`ui::Step::done`]) already reports the same thing.
fn is_cargo_finished_line(raw: &str) -> bool {
    strip_ansi(raw).trim().starts_with("Finished `")
}

#[cfg(kani)]
mod kani_proofs {
    use super::*;

    /// Proves `strip_ansi`/`cargo_compiling_crate_name`/`is_cargo_finished_line`
    /// never panic for any valid UTF-8 input — including adversarial
    /// ANSI/unicode sequences a real cargo subprocess could plausibly emit,
    /// not just the well-formed lines these functions are designed around.
    /// A small symbolic byte buffer validated as UTF-8 (Kani explores every
    /// valid encoding within it) stands in for arbitrary subprocess output.
    #[kani::proof]
    #[kani::unwind(10)]
    fn prove_build_line_parsers_never_panic() {
        let bytes: [u8; 12] = kani::any();
        if let Ok(s) = core::str::from_utf8(&bytes) {
            let _ = strip_ansi(s);
            let _ = cargo_compiling_crate_name(s);
            let _ = is_cargo_finished_line(s);
        }
    }
}
