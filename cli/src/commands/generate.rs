//! `naclac generate` — regenerates the IDL and/or client SDK(s) from source.
//! This is the single, canonical home for that logic: `naclac build` calls
//! into it too (after `cargo build-sbf` succeeds) rather than keeping its
//! own separate copy, so there is exactly one implementation of "what does
//! regenerating a program's IDL mean" no matter which command triggers it.
//! Splitting IDL generation (compiles the program with its `idl-build`
//! feature) from client generation
//! (reads an already-written IDL JSON) also means either can run on its
//! own — regenerating the SDK after an app-level template change doesn't
//! require rebuilding the on-chain program, and regenerating the IDL
//! doesn't require the client-gen step to also run.

use {
    crate::{
        commands::{programs, workspace},
        ui,
    },
    serde_json::Value,
    std::{
        collections::HashSet,
        fs,
        io::Read,
        path::{Path, PathBuf},
        process::{Command, Stdio},
        sync::OnceLock,
    },
};

/// Which client SDK(s) to (re)generate. `None` means both.
#[derive(Clone, Copy)]
pub enum ClientKind {
    Rust,
    Typescript,
}

/// A program's runtime and account-data layout as its default features
/// select them, as cargo resolves them — the layout decides what IDL and
/// client are generated, the runtime only labels the build.
pub struct ProgramFeatures {
    solana_runtime: bool,
    layout: naclac_idl::Serialization,
}

impl ProgramFeatures {
    pub fn serialization(&self) -> naclac_idl::Serialization {
        self.layout
    }

    pub fn label(&self) -> String {
        let runtime = if self.solana_runtime {
            "solana"
        } else {
            "pinocchio"
        };
        let layout = match self.layout {
            naclac_idl::Serialization::Bytemuck => "bytemuck",
            naclac_idl::Serialization::Wincode => "wincode",
            naclac_idl::Serialization::Borsh => "borsh",
        };
        format!("{runtime}, {layout}")
    }
}

/// `cargo metadata` of the workspace, run once per process: its dependency
/// resolution is what cargo itself enables for every package.
fn workspace_metadata(workspace_root: &Path) -> Result<&'static Value, String> {
    static METADATA: OnceLock<Value> = OnceLock::new();
    if let Some(metadata) = METADATA.get() {
        return Ok(metadata);
    }
    let manifest_path = workspace_root.join("Cargo.toml");
    let output = Command::new("cargo")
        .arg("metadata")
        .arg("--format-version")
        .arg("1")
        .arg("--manifest-path")
        .arg(&manifest_path)
        .output()
        .map_err(|e| format!("could not run `cargo metadata`: {e}"))?;
    if !output.status.success() {
        return Err(format!(
            "`cargo metadata` failed for {}: {}",
            manifest_path.display(),
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    let metadata = serde_json::from_slice(&output.stdout)
        .map_err(|e| format!("`cargo metadata` printed invalid JSON: {e}"))?;
    Ok(METADATA.get_or_init(|| metadata))
}

/// The features cargo enables by default for the workspace package whose
/// manifest is `program_dir/Cargo.toml`.
fn default_features(workspace_root: &Path, program_dir: &Path) -> Result<HashSet<String>, String> {
    let metadata = workspace_metadata(workspace_root)?;
    let manifest = fs::canonicalize(program_dir.join("Cargo.toml"))
        .map_err(|e| format!("cannot read {}/Cargo.toml: {e}", program_dir.display()))?;
    let id = metadata
        .get("packages")
        .and_then(Value::as_array)
        .ok_or("`cargo metadata` did not list the workspace's packages")?
        .iter()
        .find(|package| {
            package
                .get("manifest_path")
                .and_then(Value::as_str)
                .and_then(|path| fs::canonicalize(path).ok())
                .is_some_and(|path| path == manifest)
        })
        .and_then(|package| package.get("id"))
        .and_then(Value::as_str)
        .ok_or_else(|| {
            format!(
                "{} is not a package of the cargo workspace at {}",
                manifest.display(),
                workspace_root.display()
            )
        })?;
    let node = metadata
        .get("resolve")
        .and_then(|resolve| resolve.get("nodes"))
        .and_then(Value::as_array)
        .ok_or("`cargo metadata` did not report a dependency resolution")?
        .iter()
        .find(|node| node.get("id").and_then(Value::as_str) == Some(id))
        .ok_or_else(|| format!("`cargo metadata` has no resolution for `{id}`"))?;
    node.get("features")
        .and_then(Value::as_array)
        .ok_or_else(|| format!("`cargo metadata` lists no features for `{id}`"))?
        .iter()
        .map(|feature| {
            feature
                .as_str()
                .map(str::to_string)
                .ok_or_else(|| format!("`cargo metadata` lists a non-string feature for `{id}`"))
        })
        .collect()
}

/// Resolves which of `bytemuck`, `borsh` and `wincode` the program enables by
/// default as cargo resolves its features, and whether it selects the
/// `solana` runtime (absent means `pinocchio`). Errors when cargo cannot
/// resolve the program, and when the program enables no layout or more than
/// one, since `naclac-lang` accepts exactly one.
pub fn detect_program_features(
    workspace_root: &Path,
    program_dir: &Path,
) -> Result<ProgramFeatures, String> {
    let enabled = default_features(workspace_root, program_dir)?;
    let layouts: Vec<(&str, naclac_idl::Serialization)> = [
        ("bytemuck", naclac_idl::Serialization::Bytemuck),
        ("borsh", naclac_idl::Serialization::Borsh),
        ("wincode", naclac_idl::Serialization::Wincode),
    ]
    .into_iter()
    .filter(|(name, _)| enabled.contains(*name))
    .collect();
    match layouts.as_slice() {
        [(_, layout)] => Ok(ProgramFeatures {
            solana_runtime: enabled.contains("solana"),
            layout: *layout,
        }),
        [] => Err(
            "its default features enable no account-data layout; add one of `bytemuck`, `borsh` \
             or `wincode` to `default` in the program's Cargo.toml"
                .to_string(),
        ),
        _ => Err(
            "its default features enable more than one account-data layout; a program uses \
             exactly one of `bytemuck`, `borsh` or `wincode`"
                .to_string(),
        ),
    }
}

/// A program's own `Cargo.toml`, read and parsed.
fn program_manifest(program_dir: &Path) -> toml::Value {
    let path = program_dir.join("Cargo.toml");
    let text = fs::read_to_string(&path)
        .unwrap_or_else(|e| ui::error(format!("Cannot read {}: {e}", path.display())));
    toml::from_str(&text)
        .unwrap_or_else(|e| ui::error(format!("{} is not valid TOML: {e}", path.display())))
}

/// Whether a program's own `Cargo.toml` lists `naclac-lang` under
/// `[dependencies]`. A program without it (a plain Solana program kept in the
/// workspace, such as a hook program a test loads) has no account-data layout,
/// IDL or generated client; it is only compiled.
pub fn depends_on_naclac_lang(program_dir: &Path) -> bool {
    program_manifest(program_dir)
        .get("dependencies")
        .and_then(|d| d.as_table())
        .is_some_and(|deps| deps.contains_key("naclac-lang"))
}

/// Gives every program that has no generated Rust client yet a minimal valid
/// stub one. `cargo` resolves every workspace member and every
/// `[dev-dependencies]` path before it compiles anything, so a single missing
/// client manifest fails the whole workspace; `naclac build` and `naclac check`
/// call this before invoking cargo. The real client replaces the stub when
/// `naclac generate` runs. The stub's feature table must declare every feature
/// the real generator's template declares (`client-gen/src/rust/mod.rs`), or a
/// program that requests one on its client dependency fails to resolve.
pub fn ensure_client_stubs(workspace_root: &Path, program_dirs: &[PathBuf]) {
    for prog_dir in program_dirs {
        if !depends_on_naclac_lang(prog_dir) {
            continue;
        }
        let Some(pname) = prog_dir.file_name().and_then(|n| n.to_str()) else {
            continue;
        };
        let pname_kebab = pname.replace('_', "-");
        let rust_client_dir = naclac_client_gen::clients_dir(workspace_root)
            .join("rust")
            .join(pname);
        if rust_client_dir.join("Cargo.toml").exists() {
            continue;
        }
        let src_dir = rust_client_dir.join("src");
        let mut result = fs::create_dir_all(&src_dir);
        if result.is_err() {
            for _ in 1..=5 {
                std::thread::sleep(std::time::Duration::from_millis(200));
                result = fs::create_dir_all(&src_dir);
                if result.is_ok() {
                    break;
                }
            }
        }
        if let Err(e) = result {
            ui::error(format!(
                "Failed to create directory {:?}: {} — often a Windows/WSL 'delete pending' \
                 lock from VS Code/rust-analyzer/a terminal; close anything touching the \
                 clients folder and retry.",
                src_dir, e
            ));
        }
        let (naclac_lang_path, naclac_client_path) =
            naclac_client_gen::naclac_dep_path_fragments(workspace_root, &rust_client_dir)
                .unwrap_or_else(|e| ui::error(e));
        let stub_cargo = format!(
            "[package]\nname = \"{pname_kebab}-client\"\nversion = \"0.1.0\"\nedition = \"2021\"\n\n[features]\ndefault = [\"offchain\"]\noffchain = [\"dep:naclac-client\"]\ncpi = [\"dep:naclac-lang\"]\nsolana = [\"naclac-lang?/solana\"]\npinocchio = [\"naclac-lang?/pinocchio\"]\nbytemuck = [\"naclac-lang?/bytemuck\"]\nborsh = [\"naclac-lang?/borsh\"]\nwincode = [\"naclac-lang?/wincode\"]\n\n[dependencies]\nnaclac-client = {{ version = \"0.1.0\", optional = true{naclac_client_path} }}\nnaclac-lang = {{ version = \"0.1.0\", optional = true, default-features = false{naclac_lang_path} }}\n"
        );
        if fs::write(rust_client_dir.join("Cargo.toml"), &stub_cargo).is_err()
            || fs::write(
                rust_client_dir.join("src/lib.rs"),
                "// Stub — will be overwritten by naclac generate\n",
            )
            .is_err()
        {
            ui::error(format!(
                "Failed to write the stub client for '{}' under {:?}.",
                pname, rust_client_dir
            ));
        }
    }
}

fn idl_build_fix(program_dir: &Path) -> String {
    format!(
        "Add `idl-build = [\"naclac-lang/idl-build\"]` under `[features]` in {}.",
        program_dir.join("Cargo.toml").display()
    )
}

/// For `naclac build`: whether the program's IDL and client SDKs can be
/// generated, i.e. its own `Cargo.toml` declares the `idl-build` feature the
/// IDL is generated from. When it does not, prints a warning naming the line
/// to add and returns `false`; the caller skips generation for that program.
pub fn can_generate(program_dir: &Path, prog_name: &str) -> bool {
    if program_declares_idl_build(program_dir) {
        return true;
    }
    ui::warn(format!(
        "'{prog_name}' does not declare the `idl-build` feature, so its IDL and client SDKs are \
         not generated. {}",
        idl_build_fix(program_dir)
    ));
    false
}

/// For the commands that generate an IDL: errors unless the program's own
/// `Cargo.toml` declares the `idl-build` feature the IDL is generated from.
pub fn require_idl_build(program_dir: &Path, prog_name: &str) {
    if !program_declares_idl_build(program_dir) {
        ui::error(format!(
            "Cannot generate the IDL of '{prog_name}': its Cargo.toml does not declare the \
             `idl-build` feature the IDL is generated from. {}",
            idl_build_fix(program_dir)
        ));
    }
}

/// Whether a program's own `Cargo.toml` declares an `idl-build` feature.
fn program_declares_idl_build(program_dir: &Path) -> bool {
    program_manifest(program_dir)
        .get("features")
        .and_then(|f| f.as_table())
        .is_some_and(|features| features.contains_key("idl-build"))
}

pub fn read_package_name(program_dir: &Path) -> String {
    program_manifest(program_dir)
        .get("package")
        .and_then(|package| package.get("name"))
        .and_then(|name| name.as_str())
        .map(str::to_string)
        .unwrap_or_else(|| {
            ui::error(format!(
                "{} has no `name` under `[package]`.",
                program_dir.join("Cargo.toml").display()
            ))
        })
}

/// Runs the target program's real, compiled `idl-build` print function and
/// returns its IDL JSON.
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
    let crate_name = read_package_name(program_dir);
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

/// The folders of the programs a command works on: `requested`, or every
/// program Naclac.toml lists when empty.
fn selected_dirs(workspace_root: &Path, requested: &[String]) -> Vec<PathBuf> {
    let config = workspace::config(workspace_root);
    programs::select(&programs::listed(&config), requested)
        .iter()
        .map(|name| programs::program_dir(workspace_root, name))
        .collect()
}

/// Regenerates and persists one program's IDL from the compiled program (its
/// `idl-build` feature; the address in it is the program's `declare_id!`),
/// checks its `metadata.serialization` against the program's Cargo.toml
/// features, and writes `artifacts/idl/<name>.json`. Returns the IDL JSON.
pub fn generate_idl_for_program(
    workspace_root: &Path,
    program_dir: &Path,
    prog_name: &str,
    features: &ProgramFeatures,
    show_output: bool,
) -> String {
    let serialization = features.serialization();
    let target_dir =
        naclac_client_gen::resolve_target_dir(workspace_root).unwrap_or_else(|e| ui::error(e));
    let idl_json_pretty =
        match generate_idl_via_idl_build(&target_dir, program_dir, prog_name, show_output) {
            Ok(result) => result,
            Err(e) => ui::error(format!("Failed to generate IDL for '{}': {}", prog_name, e)),
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

    let idl_dir = naclac_client_gen::idl_dir(workspace_root);
    fs::create_dir_all(&idl_dir).unwrap();
    let idl_path = idl_dir.join(format!("{}.json", prog_name));
    fs::write(&idl_path, &idl_json_pretty).unwrap();
    idl_json_pretty
}

/// `naclac generate idl` — regenerates the IDL JSON only. No
/// `cargo build-sbf`, no client SDK regeneration.
pub fn execute_idl(requested: &[String], show_output: bool) {
    let workspace_root = workspace::root();
    let program_dirs = selected_dirs(&workspace_root, requested);

    for program_dir in &program_dirs {
        if !depends_on_naclac_lang(program_dir) {
            continue;
        }
        let prog_name = program_dir
            .file_name()
            .unwrap()
            .to_str()
            .unwrap()
            .to_string();
        require_idl_build(program_dir, &prog_name);
        // A real `idl-build` compile can take well over a minute; with
        // `show_output` its cargo output is inherited straight to the
        // terminal, which would otherwise fight a redrawing spinner for the
        // same line — skip the spinner in that case, matching how
        // `build.rs` handles `--show-output` for `cargo build-sbf`.
        let step = if show_output {
            None
        } else {
            Some(ui::Step::start(format!(
                "Generating IDL ({})...",
                prog_name
            )))
        };
        let program_features = detect_program_features(&workspace_root, program_dir)
            .unwrap_or_else(|e| ui::error(format!("Cannot generate '{}': {}", prog_name, e)));
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
/// (`artifacts/idl/<name>.json`) of each program in `requested` (every program
/// Naclac.toml lists when empty) that depends on naclac-lang — `target` selects
/// Rust only, TypeScript only, or both when `None`.
pub fn execute_client(requested: &[String], target: Option<ClientKind>) {
    let workspace_root = workspace::root();
    let idl_dir = naclac_client_gen::idl_dir(&workspace_root);
    let program_dirs: Vec<PathBuf> = selected_dirs(&workspace_root, requested)
        .into_iter()
        .filter(|dir| depends_on_naclac_lang(dir))
        .collect();
    if program_dirs.is_empty() {
        ui::error(
            "None of the selected programs depend on naclac-lang, so there is no client to \
             generate.",
        );
    }

    let kind_label = match target {
        Some(ClientKind::Rust) => "Rust client SDK",
        Some(ClientKind::Typescript) => "TypeScript client SDK",
        None => "Rust + TypeScript client SDKs",
    };

    for program_dir in &program_dirs {
        let program_name = program_dir
            .file_name()
            .unwrap()
            .to_str()
            .unwrap()
            .to_string();
        let json_path = idl_dir.join(format!("{}.json", program_name));
        let content = fs::read_to_string(&json_path).unwrap_or_else(|e| {
            ui::error(format!(
                "Cannot read the IDL of '{}' at {}: {}. Run `naclac generate idl` (or \
                 `naclac build`) first.",
                program_name,
                json_path.display(),
                e
            ))
        });

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

/// `naclac generate` with no subcommand — regenerates the IDL (compiling each
/// program with its `idl-build` feature), then both client SDKs from the
/// freshly-written IDL, all in one run (no on-chain build).
pub fn execute_all(requested: &[String], show_output: bool) {
    let workspace_root = workspace::root();
    let program_dirs = selected_dirs(&workspace_root, requested);

    for program_dir in &program_dirs {
        if !depends_on_naclac_lang(program_dir) {
            continue;
        }
        let prog_name = program_dir
            .file_name()
            .unwrap()
            .to_str()
            .unwrap()
            .to_string();
        require_idl_build(program_dir, &prog_name);
        let program_features = detect_program_features(&workspace_root, program_dir)
            .unwrap_or_else(|e| ui::error(format!("Cannot generate '{}': {}", prog_name, e)));
        let step = if show_output {
            None
        } else {
            Some(ui::Step::start(format!(
                "Generating IDL ({})...",
                prog_name
            )))
        };
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

    execute_client(requested, None);
}
