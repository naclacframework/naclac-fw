//! `naclac generate` — regenerates the IDL and/or client SDK(s) from source.
//! This is the single, canonical home for that logic: `naclac build` calls
//! into it too (after `cargo build-sbf` succeeds) rather than keeping its
//! own separate copy, so there is exactly one implementation of "what does
//! regenerating a program's IDL mean" no matter which command triggers it.
//! Splitting IDL generation (pure source parsing) from client generation
//! (reads an already-written IDL JSON) also means either can run on its
//! own — regenerating the SDK after an app-level template change doesn't
//! require rebuilding the on-chain program, and regenerating the IDL
//! doesn't require the client-gen step to also run.

use {
    crate::ui,
    std::{
        fs,
        io::Read,
        path::{Path, PathBuf},
        process::{Command, Stdio},
    },
};

/// Which client SDK(s) to (re)generate. `None` means both.
#[derive(Clone, Copy)]
pub enum ClientKind {
    Rust,
    Typescript,
}

/// A program's build-relevant feature flags, read once from its
/// `Cargo.toml` — shared by `naclac build` (which needs `use_pinocchio`
/// for its `cargo build-sbf` arguments) and IDL generation (which needs
/// `is_zero_copy`), so the same file only gets parsed once.
pub struct ProgramFeatures {
    pub use_pinocchio: bool,
    /// True only for the program's *raw* zero-copy/bytemuck representation
    /// (Pinocchio, or neither `borsh` nor `wincode` requested) — there are
    /// three account-data representations in this framework (Borsh, Wincode,
    /// raw zero-copy), not two, so this is `use_pinocchio || (!use_borsh &&
    /// !use_wincode)`, not a plain `!use_borsh`.
    pub is_zero_copy: bool,
    /// True when the program requests the `wincode` feature (and isn't
    /// Pinocchio, which is always raw zero-copy regardless of this flag).
    pub use_wincode: bool,
}

impl ProgramFeatures {
    pub fn serialization(&self) -> naclac_idl::Serialization {
        if self.is_zero_copy {
            naclac_idl::Serialization::Bytemuck
        } else if self.use_wincode {
            naclac_idl::Serialization::Wincode
        } else {
            naclac_idl::Serialization::Borsh
        }
    }
}

pub fn detect_program_features(program_dir: &Path) -> ProgramFeatures {
    let cargo_toml_path = program_dir.join("Cargo.toml");
    let mut use_pinocchio = false;
    let mut use_borsh = false;
    let mut use_wincode = false;
    let mut declares_pinocchio_feature = false;
    if let Ok(content) = fs::read_to_string(&cargo_toml_path) {
        if let Ok(parsed) = toml::from_str::<toml::Value>(&content) {
            if let Some(features) = parsed.get("features").and_then(|f| f.as_table()) {
                if let Some(default_feats) = features.get("default").and_then(|d| d.as_array()) {
                    use_pinocchio = default_feats
                        .iter()
                        .any(|v| v.as_str() == Some("pinocchio"));
                    use_borsh = default_feats.iter().any(|v| v.as_str() == Some("borsh"));
                    use_wincode = default_feats.iter().any(|v| v.as_str() == Some("wincode"));
                }
                declares_pinocchio_feature = features.contains_key("pinocchio");
            }
            // `borsh`/`wincode` may also be requested directly on the
            // naclac-lang dependency rather than declared as the program's
            // own default feature.
            if !use_borsh {
                use_borsh = parsed
                    .get("dependencies")
                    .and_then(|d| d.get("naclac-lang"))
                    .and_then(|dep| dep.get("features"))
                    .and_then(|f| f.as_array())
                    .map(|arr| arr.iter().any(|v| v.as_str() == Some("borsh")))
                    .unwrap_or(false);
            }
            if !use_wincode {
                use_wincode = parsed
                    .get("dependencies")
                    .and_then(|d| d.get("naclac-lang"))
                    .and_then(|dep| dep.get("features"))
                    .and_then(|f| f.as_array())
                    .map(|arr| arr.iter().any(|v| v.as_str() == Some("wincode")))
                    .unwrap_or(false);
            }
            if !use_borsh && !use_wincode {
                use_pinocchio |= declares_pinocchio_feature;
            }
        }
    }
    ProgramFeatures {
        use_pinocchio,
        is_zero_copy: use_pinocchio || (!use_borsh && !use_wincode),
        use_wincode,
    }
}

/// Whether a program's own `Cargo.toml` declares an `idl-build` feature —
/// the opt-in signal for using the real-compilation `idl-build` mechanism
/// instead of the AST-walker (`naclac_idl::generate_idl`) for that program.
pub fn program_declares_idl_build(program_dir: &Path) -> bool {
    let Ok(content) = fs::read_to_string(program_dir.join("Cargo.toml")) else {
        return false;
    };
    let Ok(parsed) = toml::from_str::<toml::Value>(&content) else {
        return false;
    };
    parsed
        .get("features")
        .and_then(|f| f.as_table())
        .is_some_and(|features| features.contains_key("idl-build"))
}

fn read_package_name(program_dir: &Path) -> Option<String> {
    let content = fs::read_to_string(program_dir.join("Cargo.toml")).ok()?;
    let parsed: toml::Value = toml::from_str(&content).ok()?;
    parsed
        .get("package")?
        .get("name")?
        .as_str()
        .map(str::to_string)
}

/// Runs the target program's real, compiled `idl-build` print function and
/// returns the same IDL JSON the AST-walker path returns, so callers don't
/// need to know which mechanism actually produced it.
///
/// The program is compiled in place, from its own manifest, as an `rlib`
/// regardless of the `crate-type` it declares. It must be built from its real
/// directory: the proc-macros locate `src/` and `Naclac.toml` through
/// `CARGO_MANIFEST_DIR`, and the program workspace's `Cargo.lock` must apply.
/// A throwaway runner source file linked against that `rlib` prints the IDL
/// JSON. The program's own `Cargo.toml`/`src` tree is never modified.
fn generate_idl_via_idl_build(
    target_dir: &Path,
    program_dir: &Path,
    prog_name: &str,
    show_output: bool,
) -> Result<String, String> {
    let crate_name = read_package_name(program_dir).unwrap_or_else(|| prog_name.to_string());
    let lib_ident = crate_name.replace('-', "_");

    let program_dir_abs = program_dir.canonicalize().map_err(|e| e.to_string())?;
    let artifact_dir = target_dir.join("__naclac_idl_build");
    let cargo_target_dir = artifact_dir.join("target");
    let runner_dir = artifact_dir.join("runner");
    if runner_dir.exists() {
        fs::remove_dir_all(&runner_dir).map_err(|e| e.to_string())?;
    }
    fs::create_dir_all(&runner_dir).map_err(|e| e.to_string())?;

    let result = (|| {
        let mut build = Command::new("cargo");
        build
            .arg("rustc")
            .arg("--manifest-path")
            .arg(program_dir_abs.join("Cargo.toml"))
            .args(["--lib", "--crate-type", "rlib", "--features", "idl-build"])
            .arg("--target-dir")
            .arg(&cargo_target_dir)
            .current_dir(&program_dir_abs);
        run_capturing_stdout(
            build,
            show_output,
            &format!("idl-build compilation for '{prog_name}'"),
        )?;

        let main_rs = runner_dir.join("main.rs");
        fs::write(
            &main_rs,
            format!("fn main() {{ {lib_ident}::__naclac_print_idl(); }}\n"),
        )
        .map_err(|e| e.to_string())?;

        let debug_dir = cargo_target_dir.join("debug");
        let runner_bin = runner_dir.join(format!("idl_runner{}", std::env::consts::EXE_SUFFIX));
        let mut link = Command::new("rustc");
        link.args(["--edition", "2021"])
            .arg(&main_rs)
            .arg("--extern")
            .arg(format!(
                "{lib_ident}={}",
                debug_dir.join(format!("lib{lib_ident}.rlib")).display()
            ))
            .arg("-L")
            .arg(format!("dependency={}", debug_dir.join("deps").display()))
            .arg("-o")
            .arg(&runner_bin);
        run_capturing_stdout(
            link,
            show_output,
            &format!("idl-build runner link for '{prog_name}'"),
        )?;

        let stdout_buf = run_capturing_stdout(
            Command::new(&runner_bin),
            show_output,
            &format!("idl-build print binary for '{prog_name}'"),
        )?;

        let value: serde_json::Value = serde_json::from_str(stdout_buf.trim())
            .map_err(|e| format!("idl-build output was not valid JSON: {e}"))?;

        naclac_idl::to_compact_pretty_json(&value)
            .map_err(|e| format!("failed to re-serialize idl-build output: {e}"))
    })();

    let _ = fs::remove_dir_all(&runner_dir);
    result
}

/// Runs `cmd` to completion and returns its stdout. Stderr is inherited when
/// `show_output` is set; otherwise it is captured and included in the error
/// returned for a non-zero exit.
fn run_capturing_stdout(mut cmd: Command, show_output: bool, what: &str) -> Result<String, String> {
    cmd.stdout(Stdio::piped()).stderr(if show_output {
        Stdio::inherit()
    } else {
        Stdio::piped()
    });

    let mut child = cmd
        .spawn()
        .map_err(|e| format!("failed to start {what}: {e}"))?;
    let mut stdout_pipe = child.stdout.take().expect("stdout was piped");
    let stderr_pipe = child.stderr.take();

    // Stdout and stderr must be drained concurrently — reading one to
    // completion before touching the other risks the child blocking on a full
    // pipe buffer for the stream nobody is draining yet.
    let stderr_thread = stderr_pipe.map(|mut pipe| {
        std::thread::spawn(move || {
            let mut buf = String::new();
            let _ = pipe.read_to_string(&mut buf);
            buf
        })
    });

    let mut stdout_buf = String::new();
    stdout_pipe
        .read_to_string(&mut stdout_buf)
        .map_err(|e| format!("failed to read stdout of {what}: {e}"))?;
    let captured_stderr = stderr_thread.map(|t| t.join().unwrap_or_default());

    let status = child
        .wait()
        .map_err(|e| format!("failed to wait on {what}: {e}"))?;
    if !status.success() {
        return Err(match captured_stderr {
            Some(stderr) => format!("{what} failed:\n{stderr}"),
            None => format!("{what} failed — see compiler output above."),
        });
    }
    Ok(stdout_buf)
}

pub fn resolve_workspace_root() -> PathBuf {
    let current_dir = std::env::current_dir().unwrap();
    if current_dir.join("Naclac.toml").exists() {
        current_dir
    } else if current_dir.join("../../Naclac.toml").exists() {
        current_dir.join("../..").canonicalize().unwrap()
    } else {
        ui::error("Could not find Naclac.toml.")
    }
}

fn target_program_dirs(workspace_root: &Path, program_id: Option<&str>) -> Vec<PathBuf> {
    let programs_dir = workspace_root.join("programs");
    if !programs_dir.exists() {
        ui::error("No valid Naclac programs found in workspace.");
    }
    fs::read_dir(&programs_dir)
        .unwrap()
        .filter_map(|entry| {
            let path = entry.unwrap().path();
            let is_target = program_id.is_none_or(|tgt| path.file_name().unwrap() == tgt);
            if path.is_dir() && path.join("src/lib.rs").exists() && is_target {
                Some(path)
            } else {
                None
            }
        })
        .collect()
}

/// Regenerates and persists one program's IDL: resolves its deploy-keypair
/// address, builds the IDL (from the compiled program when it declares
/// `idl-build`, from source otherwise), checks its `metadata.serialization`
/// against the program's Cargo.toml features, and writes
/// `target/idl/<name>.json`. Returns the IDL JSON.
pub fn generate_idl_for_program(
    workspace_root: &Path,
    program_dir: &Path,
    prog_name: &str,
    features: &ProgramFeatures,
    show_output: bool,
) -> String {
    let serialization = features.serialization();
    let target_dir = naclac_client_gen::resolve_target_dir(workspace_root);
    let keypair_path = target_dir
        .join("deploy")
        .join(format!("{}-keypair.json", prog_name));
    if !keypair_path.exists() {
        ui::error(format!(
            "No deploy keypair found for '{}' — run `naclac build` at least once first.",
            prog_name
        ));
    }
    let address_output = Command::new("solana-keygen")
        .arg("pubkey")
        .arg(&keypair_path)
        .output()
        .expect("Failed to run solana-keygen");
    let actual_address = String::from_utf8_lossy(&address_output.stdout)
        .trim()
        .to_string();

    let idl_json_pretty = if program_declares_idl_build(program_dir) {
        match generate_idl_via_idl_build(&target_dir, program_dir, prog_name, show_output) {
            Ok(result) => result,
            Err(e) => ui::error(format!("Failed to generate IDL for '{}': {}", prog_name, e)),
        }
    } else {
        match naclac_idl::generate_idl(
            program_dir,
            prog_name,
            &actual_address,
            env!("CARGO_PKG_VERSION"),
            serialization,
        ) {
            Ok(result) => result,
            Err(e) => ui::error(format!("Failed to generate IDL for '{}': {}", prog_name, e)),
        }
    };

    let written_serialization = serde_json::from_str::<serde_json::Value>(&idl_json_pretty)
        .ok()
        .and_then(|idl| idl.pointer("/metadata/serialization").cloned());
    if written_serialization != serde_json::to_value(serialization).ok() {
        ui::error(format!(
            "The IDL for '{}' says its serialization is {:?} but its Cargo.toml features say {:?}. \
             The features the program was compiled with and the ones in Cargo.toml must agree.",
            prog_name, written_serialization, serialization
        ));
    }

    let target_idl_dir = target_dir.join("idl");
    fs::create_dir_all(&target_idl_dir).unwrap();
    let idl_path = target_idl_dir.join(format!("{}.json", prog_name));
    fs::write(&idl_path, &idl_json_pretty).unwrap();
    idl_json_pretty
}

/// `naclac generate idl` — regenerates the IDL JSON only. No
/// `cargo build-sbf`, no client SDK regeneration.
pub fn execute_idl(program_id: Option<&str>, show_output: bool) {
    let workspace_root = resolve_workspace_root();
    let program_dirs = target_program_dirs(&workspace_root, program_id);
    if program_dirs.is_empty() {
        ui::error("No matching program found.");
    }

    for program_dir in &program_dirs {
        let prog_name = program_dir
            .file_name()
            .unwrap()
            .to_str()
            .unwrap()
            .to_string();
        // A real `idl-build` compile can take well over a minute; with
        // `show_output` its cargo output is inherited straight to the
        // terminal, which would otherwise fight a redrawing spinner for the
        // same line — skip the spinner in that case, matching how
        // `build.rs` handles `--show-output` for `cargo build-sbf`.
        let use_idl_build = program_declares_idl_build(program_dir) && show_output;
        let step = if use_idl_build {
            None
        } else {
            Some(ui::Step::start(format!(
                "Generating IDL ({})...",
                prog_name
            )))
        };
        let program_features = detect_program_features(program_dir);
        generate_idl_for_program(
            &workspace_root,
            program_dir,
            &prog_name,
            &program_features,
            show_output,
        );
        match step {
            Some(step) => step.done(format!("IDL written ({})", prog_name)),
            None => ui::success(format!("IDL written ({})", prog_name)),
        }
    }
}

/// Regenerates client SDK(s) from the IDL JSON already on disk
/// (`target/idl/<name>.json`) — `target` selects Rust only, TypeScript only,
/// or both when `None`.
pub fn execute_client(program_id: Option<&str>, target: Option<ClientKind>) {
    let workspace_root = resolve_workspace_root();
    let target_idl_dir = naclac_client_gen::resolve_target_dir(&workspace_root).join("idl");
    if !target_idl_dir.exists() {
        ui::error(
            "target/idl does not exist — run `naclac generate idl` (or `naclac build`) first.",
        );
    }

    let mut target_json_paths = Vec::new();

    if let Some(pid) = program_id {
        for entry in fs::read_dir(&target_idl_dir).unwrap().flatten() {
            let path = entry.path();
            if path.extension().unwrap_or_default() == "json" {
                let content = fs::read_to_string(&path).unwrap();
                let idl: serde_json::Value = serde_json::from_str(&content).unwrap();
                let program_name = idl
                    .get("metadata")
                    .and_then(|m| m.get("name"))
                    .and_then(|n| n.as_str())
                    .unwrap_or_else(|| path.file_stem().unwrap().to_str().unwrap())
                    .to_string();
                // Exact match only — a substring check here (e.g. matching
                // "accounts_constraints" against "accounts_constraints_borsh.json")
                // would regenerate the wrong sibling program's client and silently
                // leave the intended one stale, since directory iteration order
                // isn't guaranteed and this loop stops at the first hit.
                if program_name == pid {
                    target_json_paths.push((path, program_name));
                    break;
                }
            }
        }
    } else {
        for entry in fs::read_dir(&target_idl_dir).unwrap().flatten() {
            let path = entry.path();
            if path.extension().unwrap_or_default() == "json" {
                let file_stem = path.file_stem().unwrap().to_string_lossy().to_string();
                target_json_paths.push((path, file_stem));
            }
        }
    }

    if target_json_paths.is_empty() {
        ui::error("No IDL found — run `naclac generate idl` (or `naclac build`) first.");
    }

    let kind_label = match target {
        Some(ClientKind::Rust) => "Rust client SDK",
        Some(ClientKind::Typescript) => "TypeScript client SDK",
        None => "Rust + TypeScript client SDKs",
    };

    for (json_path, program_name) in target_json_paths {
        let content = fs::read_to_string(&json_path).unwrap();

        let step = ui::Step::start(format!("Generating {} ({})...", kind_label, program_name));
        let res: Result<(), Box<dyn std::error::Error>> = match target {
            Some(ClientKind::Rust) => {
                naclac_client_gen::rust::generate_rust_sdk(&content, &program_name, &workspace_root)
            }
            Some(ClientKind::Typescript) => naclac_client_gen::typescript::generate_typescript_sdk(
                &content,
                &program_name,
                &workspace_root,
            ),
            None => naclac_client_gen::generate_sdk(&content, &program_name, &workspace_root),
        };

        match res {
            Ok(()) => step.done(format!("{} written ({})", kind_label, program_name)),
            Err(e) => step.fail(format!(
                "{} generation failed for '{}': {}",
                kind_label, program_name, e
            )),
        }
    }
}

/// `naclac generate` with no subcommand — regenerates the IDL from source,
/// then both client SDKs from the freshly-written IDL, all in one run (no
/// on-chain build).
pub fn execute_all(program_id: Option<&str>, show_output: bool) {
    let workspace_root = resolve_workspace_root();
    let program_dirs = target_program_dirs(&workspace_root, program_id);
    if program_dirs.is_empty() {
        ui::error("No matching program found.");
    }

    for program_dir in &program_dirs {
        let prog_name = program_dir
            .file_name()
            .unwrap()
            .to_str()
            .unwrap()
            .to_string();
        let use_idl_build = program_declares_idl_build(program_dir) && show_output;
        let step = if use_idl_build {
            None
        } else {
            Some(ui::Step::start(format!(
                "Generating IDL ({})...",
                prog_name
            )))
        };
        let program_features = detect_program_features(program_dir);
        generate_idl_for_program(
            &workspace_root,
            program_dir,
            &prog_name,
            &program_features,
            show_output,
        );
        match step {
            Some(step) => step.done(format!("IDL written ({})", prog_name)),
            None => ui::success(format!("IDL written ({})", prog_name)),
        }
    }

    execute_client(program_id, None);
}
