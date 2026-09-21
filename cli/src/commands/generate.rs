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

use crate::ui;
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

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
    /// True when the program is Pinocchio, or when it isn't requesting the
    /// `borsh` feature at all — there is no third representation for
    /// account data in this framework, so `!borsh` is a complete signal,
    /// not a heuristic.
    pub is_zero_copy: bool,
}

pub fn detect_program_features(program_dir: &Path) -> ProgramFeatures {
    let cargo_toml_path = program_dir.join("Cargo.toml");
    let mut use_pinocchio = false;
    let mut use_borsh = false;
    if let Ok(content) = fs::read_to_string(&cargo_toml_path) {
        if let Ok(parsed) = toml::from_str::<toml::Value>(&content) {
            if let Some(features) = parsed.get("features").and_then(|f| f.as_table()) {
                if let Some(default_feats) = features.get("default").and_then(|d| d.as_array()) {
                    use_pinocchio = default_feats
                        .iter()
                        .any(|v| v.as_str() == Some("pinocchio"));
                    use_borsh = default_feats.iter().any(|v| v.as_str() == Some("borsh"));
                }
                if !use_pinocchio {
                    use_pinocchio = features.contains_key("pinocchio");
                }
            }
            // `borsh` may also be requested directly on the naclac-lang dependency
            // rather than declared as the program's own default feature.
            if !use_borsh {
                use_borsh = parsed
                    .get("dependencies")
                    .and_then(|d| d.get("naclac-lang"))
                    .and_then(|dep| dep.get("features"))
                    .and_then(|f| f.as_array())
                    .map(|arr| arr.iter().any(|v| v.as_str() == Some("borsh")))
                    .unwrap_or(false);
            }
        }
    }
    ProgramFeatures {
        use_pinocchio,
        is_zero_copy: use_pinocchio || !use_borsh,
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
/// returns the same `(idl_json_pretty, alloc_event_names)` shape the
/// AST-walker path returns, so callers don't need to know which mechanism
/// actually produced it.
///
/// The print binary lives in a throwaway, self-contained runner crate —
/// its own `[workspace]` marker, so it never becomes a member of the
/// caller's real workspace — created under the resolved `target/`
/// directory just for this run and deleted again once the JSON has been
/// captured. The target program's own `Cargo.toml`/`src` tree is never
/// touched.
fn generate_idl_via_idl_build(
    target_dir: &Path,
    program_dir: &Path,
    prog_name: &str,
    show_output: bool,
) -> Result<(String, Vec<String>), String> {
    let crate_name = read_package_name(program_dir).unwrap_or_else(|| prog_name.to_string());
    let lib_ident = crate_name.replace('-', "_");

    let runner_dir = target_dir.join("__naclac_idl_build");
    if runner_dir.exists() {
        fs::remove_dir_all(&runner_dir).map_err(|e| e.to_string())?;
    }
    fs::create_dir_all(runner_dir.join("src/bin")).map_err(|e| e.to_string())?;

    let program_dir_abs = program_dir.canonicalize().map_err(|e| e.to_string())?;

    let mut manifest = toml::map::Map::new();
    manifest.insert(
        "workspace".into(),
        toml::Value::Table(toml::map::Map::new()),
    );
    let mut package = toml::map::Map::new();
    package.insert(
        "name".into(),
        toml::Value::String("__naclac_idl_build".into()),
    );
    package.insert("version".into(), toml::Value::String("0.1.0".into()));
    package.insert("edition".into(), toml::Value::String("2021".into()));
    package.insert("publish".into(), toml::Value::Boolean(false));
    manifest.insert("package".into(), toml::Value::Table(package));

    let mut bin = toml::map::Map::new();
    bin.insert("name".into(), toml::Value::String(crate_name.clone()));
    bin.insert(
        "path".into(),
        toml::Value::String(format!("src/bin/{crate_name}.rs")),
    );
    bin.insert(
        "required-features".into(),
        toml::Value::Array(vec![toml::Value::String("idl-build".into())]),
    );
    manifest.insert("bin".into(), toml::Value::Array(vec![toml::Value::Table(bin)]));

    let mut features = toml::map::Map::new();
    features.insert(
        "idl-build".into(),
        toml::Value::Array(vec![toml::Value::String(format!(
            "{crate_name}/idl-build"
        ))]),
    );
    manifest.insert("features".into(), toml::Value::Table(features));

    let mut dep = toml::map::Map::new();
    dep.insert(
        "path".into(),
        toml::Value::String(program_dir_abs.display().to_string()),
    );
    let mut dependencies = toml::map::Map::new();
    dependencies.insert(crate_name.clone(), toml::Value::Table(dep));
    manifest.insert("dependencies".into(), toml::Value::Table(dependencies));

    let cargo_toml_content = toml::to_string(&toml::Value::Table(manifest)).map_err(|e| e.to_string())?;
    fs::write(runner_dir.join("Cargo.toml"), cargo_toml_content).map_err(|e| e.to_string())?;

    let bin_rs = format!("fn main() {{ {lib_ident}::__naclac_print_idl(); }}\n");
    fs::write(
        runner_dir.join("src/bin").join(format!("{crate_name}.rs")),
        bin_rs,
    )
    .map_err(|e| e.to_string())?;

    let mut cmd = Command::new("cargo");
    cmd.args(["run", "--bin", &crate_name, "--features", "idl-build"])
        .current_dir(&runner_dir)
        .stdout(Stdio::piped())
        .stderr(if show_output {
            Stdio::inherit()
        } else {
            Stdio::piped()
        });

    let result = (|| {
        let mut child = cmd.spawn().map_err(|e| format!("failed to run cargo: {e}"))?;
        let mut stdout_pipe = child.stdout.take().expect("stdout was piped");
        let stderr_pipe = child.stderr.take();

        // Cargo's own compile output (potentially many lines) goes to
        // stderr; the print binary's one-shot JSON goes to stdout. Both
        // must be drained concurrently — reading one to completion before
        // touching the other risks the child blocking on a full pipe
        // buffer for the stream nobody is draining yet.
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
            .map_err(|e| format!("failed to read idl-build stdout: {e}"))?;
        let captured_stderr = stderr_thread.map(|t| t.join().unwrap_or_default());

        let status = child
            .wait()
            .map_err(|e| format!("failed to wait on cargo: {e}"))?;
        if !status.success() {
            return Err(match captured_stderr {
                Some(stderr) => {
                    format!("idl-build compilation failed for '{}':\n{}", prog_name, stderr)
                }
                None => format!(
                    "idl-build compilation failed for '{}' — see compiler output above.",
                    prog_name
                ),
            });
        }

        let mut value: serde_json::Value = serde_json::from_str(stdout_buf.trim())
            .map_err(|e| format!("idl-build output was not valid JSON: {e}"))?;

        let alloc_event_names = value
            .as_object_mut()
            .and_then(|map| map.remove("__allocEvents"))
            .and_then(|v| v.as_array().cloned())
            .unwrap_or_default()
            .into_iter()
            .filter_map(|v| v.as_str().map(str::to_string))
            .collect();

        let idl_json_pretty = naclac_idl::to_compact_pretty_json(&value)
            .map_err(|e| format!("failed to re-serialize idl-build output: {e}"))?;

        Ok((idl_json_pretty, alloc_event_names))
    })();

    let _ = fs::remove_dir_all(&runner_dir);
    result
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

fn zero_copy_marker_path(workspace_root: &Path, prog_name: &str) -> PathBuf {
    naclac_client_gen::resolve_target_dir(workspace_root).join(format!(".{}-zero-copy", prog_name))
}

/// Regenerates and persists one program's IDL: resolves its deploy-keypair
/// address, calls `naclac_idl::generate_idl`, writes `target/idl/<name>.json`,
/// and — when `is_zero_copy` — writes the zero-copy marker file with the
/// real `#[event(alloc)]` names (not a placeholder), consumed by client-gen
/// afterward. Does **not** remove the marker; the caller owns that, since
/// whether it should be removed immediately or kept around for a following
/// client-gen step depends on the caller's own flow (see `execute_idl` vs
/// `execute_all` below).
pub fn generate_idl_for_program(
    workspace_root: &Path,
    program_dir: &Path,
    prog_name: &str,
    is_zero_copy: bool,
    show_output: bool,
) -> (String, Vec<String>) {
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

    let (idl_json_pretty, alloc_event_names) = if program_declares_idl_build(program_dir) {
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
            is_zero_copy,
        ) {
            Ok(result) => result,
            Err(e) => ui::error(format!("Failed to generate IDL for '{}': {}", prog_name, e)),
        }
    };

    let target_idl_dir = target_dir.join("idl");
    fs::create_dir_all(&target_idl_dir).unwrap();
    let idl_path = target_idl_dir.join(format!("{}.json", prog_name));
    fs::write(&idl_path, &idl_json_pretty).unwrap();

    if is_zero_copy {
        let marker_path = zero_copy_marker_path(workspace_root, prog_name);
        let _ = fs::write(&marker_path, alloc_event_names.join(","));
    }

    (idl_json_pretty, alloc_event_names)
}

/// `naclac generate idl` — regenerates the IDL JSON from source only. No
/// `cargo build-sbf`, no client SDK regeneration. Self-contained: writes
/// its own zero-copy marker (if any) and removes it again before returning,
/// since nothing else runs in this same command to consume it.
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
            Some(ui::Step::start(format!("Generating IDL ({})...", prog_name)))
        };
        let program_features = detect_program_features(program_dir);
        generate_idl_for_program(
            &workspace_root,
            program_dir,
            &prog_name,
            program_features.is_zero_copy,
            show_output,
        );
        if program_features.is_zero_copy {
            let _ = fs::remove_file(zero_copy_marker_path(&workspace_root, &prog_name));
        }
        match step {
            Some(step) => step.done(format!("IDL written ({})", prog_name)),
            None => ui::success(format!("IDL written ({})", prog_name)),
        }
    }
}

/// Regenerates client SDK(s) from the IDL JSON already on disk
/// (`target/idl/<name>.json`) — `target` selects Rust only, TypeScript only,
/// or both when `None`.
///
/// Marker-file handling: if a zero-copy marker already exists for a program
/// (written moments ago by `generate_idl_for_program`, e.g. via
/// `execute_all` below), it is left completely alone — its real
/// `#[event(alloc)]` names must reach client-gen unmodified. A marker is
/// only created (and cleaned up again) here when none existed at all, as a
/// same-run fallback for a standalone `naclac generate client` invocation
/// with no preceding IDL step.
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
                // Just a cheap parse to see if it matches pid
                let idl: serde_json::Value = serde_json::from_str(&content).unwrap();
                if content.contains(pid) {
                    let program_name = idl
                        .get("metadata")
                        .and_then(|m| m.get("name"))
                        .and_then(|n| n.as_str())
                        .unwrap_or_else(|| path.file_stem().unwrap().to_str().unwrap())
                        .to_string();
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

        let program_dir = workspace_root.join("programs").join(&program_name);
        let marker_path = zero_copy_marker_path(&workspace_root, &program_name);
        let marker_existed = marker_path.exists();
        let created_placeholder = if !marker_existed
            && program_dir.exists()
            && detect_program_features(&program_dir).is_zero_copy
        {
            let _ = fs::write(&marker_path, "");
            true
        } else {
            false
        };

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

        if created_placeholder {
            let _ = fs::remove_file(&marker_path);
        }

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
/// on-chain build). The zero-copy markers written during the IDL step are
/// kept alive across the client step (see `execute_client`'s doc comment)
/// and only removed once both steps are done.
pub fn execute_all(program_id: Option<&str>, show_output: bool) {
    let workspace_root = resolve_workspace_root();
    let program_dirs = target_program_dirs(&workspace_root, program_id);
    if program_dirs.is_empty() {
        ui::error("No matching program found.");
    }

    let mut zero_copy_programs = Vec::new();
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
            Some(ui::Step::start(format!("Generating IDL ({})...", prog_name)))
        };
        let program_features = detect_program_features(program_dir);
        generate_idl_for_program(
            &workspace_root,
            program_dir,
            &prog_name,
            program_features.is_zero_copy,
            show_output,
        );
        match step {
            Some(step) => step.done(format!("IDL written ({})", prog_name)),
            None => ui::success(format!("IDL written ({})", prog_name)),
        }
        if program_features.is_zero_copy {
            zero_copy_programs.push(prog_name);
        }
    }

    execute_client(program_id, None);

    for prog_name in zero_copy_programs {
        let _ = fs::remove_file(zero_copy_marker_path(&workspace_root, &prog_name));
    }
}
