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
use std::path::{Path, PathBuf};
use std::process::Command;

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

    let (idl_json_pretty, alloc_event_names) = match naclac_idl::generate_idl(
        program_dir,
        prog_name,
        &actual_address,
        env!("CARGO_PKG_VERSION"),
        is_zero_copy,
    ) {
        Ok(result) => result,
        Err(e) => ui::error(format!("Failed to generate IDL for '{}': {}", prog_name, e)),
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
pub fn execute_idl(program_id: Option<&str>) {
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
        let step = ui::Step::start(format!("Generating IDL ({})...", prog_name));
        let program_features = detect_program_features(program_dir);
        generate_idl_for_program(
            &workspace_root,
            program_dir,
            &prog_name,
            program_features.is_zero_copy,
        );
        if program_features.is_zero_copy {
            let _ = fs::remove_file(zero_copy_marker_path(&workspace_root, &prog_name));
        }
        step.done(format!("IDL written ({})", prog_name));
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
pub fn execute_all(program_id: Option<&str>) {
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
        let step = ui::Step::start(format!("Generating IDL ({})...", prog_name));
        let program_features = detect_program_features(program_dir);
        generate_idl_for_program(
            &workspace_root,
            program_dir,
            &prog_name,
            program_features.is_zero_copy,
        );
        step.done(format!("IDL written ({})", prog_name));
        if program_features.is_zero_copy {
            zero_copy_programs.push(prog_name);
        }
    }

    execute_client(program_id, None);

    for prog_name in zero_copy_programs {
        let _ = fs::remove_file(zero_copy_marker_path(&workspace_root, &prog_name));
    }
}
