//! Drives `trybuild` over the fixtures that apply to the one runtime and one
//! account-data layout this run was built with. Run it once per combination:
//! `cargo test --test compile_fail --no-default-features --features RUNTIME,LAYOUT`
//! with `RUNTIME` one of `solana`/`pinocchio` and `LAYOUT` one of
//! `bytemuck`/`borsh`/`wincode` (`pinocchio,borsh` does not exist: Borsh is
//! Solana-runtime only). Two further configurations run only their own scope
//! folders: `RUNTIME,LAYOUT,idl-build` takes `idl-build` and `idl-build-LAYOUT`,
//! and `RUNTIME,no-layout` takes `no-layout` and `RUNTIME-no-layout`.
//!
//! Each fixture directory (`tests/fail` and `tests/pass`) holds the fixtures
//! of every configuration at its top level, and one sub-folder per narrower
//! scope: `solana`, `pinocchio`, `bytemuck`, `borsh`, `wincode`, `heap`
//! (Borsh and Wincode, where the check and its message are the same), and the
//! five exact combinations such as `solana-bytemuck`. A run takes the top level
//! plus every scope that matches its configuration. A fixture belongs in a
//! scope when the check it exercises is emitted only there.
//!
//! `fail/**/*.rs` must fail to compile, and each has a committed `.stderr`
//! snapshot that `trybuild` compares exactly; without one it writes the actual
//! output to `wip/` and the suite fails until that file is reviewed and moved
//! next to the fixture (`TRYBUILD=overwrite` regenerates it). `pass/**/*.rs`
//! are the valid counterparts, which catch a check becoming too strict.
//! `tests/coverage` checks that every error message the macros can emit has a
//! fixture here.

#[cfg(all(feature = "solana", feature = "pinocchio"))]
compile_error!("Naclac Error: enable exactly one runtime feature, `solana` or `pinocchio`.");
#[cfg(not(any(feature = "solana", feature = "pinocchio")))]
compile_error!(
    "Naclac Error: enable one runtime feature, `solana` or `pinocchio` (pass `--no-default-features --features RUNTIME,LAYOUT`)."
);
#[cfg(any(
    all(feature = "bytemuck", feature = "borsh"),
    all(feature = "bytemuck", feature = "wincode"),
    all(feature = "borsh", feature = "wincode")
))]
compile_error!("Naclac Error: enable exactly one layout feature: `bytemuck`, `borsh` or `wincode`.");
#[cfg(not(any(
    feature = "bytemuck",
    feature = "borsh",
    feature = "wincode",
    feature = "no-layout"
)))]
compile_error!(
    "Naclac Error: enable one layout feature: `bytemuck`, `borsh` or `wincode` (pass `--no-default-features --features RUNTIME,LAYOUT`)."
);
#[cfg(all(
    feature = "no-layout",
    any(feature = "bytemuck", feature = "borsh", feature = "wincode")
))]
compile_error!("Naclac Error: `no-layout` excludes every layout feature.");
#[cfg(all(feature = "no-layout", feature = "idl-build"))]
compile_error!("Naclac Error: `no-layout` and `idl-build` are separate configurations.");
#[cfg(all(feature = "pinocchio", feature = "borsh"))]
compile_error!("Naclac Error: the `borsh` layout needs the `solana` runtime, not `pinocchio`.");

use std::path::{Path, PathBuf};

fn runtime() -> &'static str {
    if cfg!(feature = "solana") {
        "solana"
    } else {
        "pinocchio"
    }
}

fn layout() -> &'static str {
    if cfg!(feature = "no-layout") {
        "no-layout"
    } else if cfg!(feature = "bytemuck") {
        "bytemuck"
    } else if cfg!(feature = "borsh") {
        "borsh"
    } else {
        "wincode"
    }
}

/// Whether the fixtures at the top of `tests/fail` and `tests/pass`, which every ordinary
/// configuration runs, apply. An `idl-build` or `no-layout` run takes only its own folders.
fn runs_top_level() -> bool {
    !cfg!(feature = "idl-build") && !cfg!(feature = "no-layout")
}

/// The sub-folders of `tests/fail` and `tests/pass` this configuration runs, after the top level.
fn scopes() -> Vec<String> {
    if cfg!(feature = "idl-build") {
        return vec!["idl-build".to_string(), format!("idl-build-{}", layout())];
    }
    if cfg!(feature = "no-layout") {
        return vec!["no-layout".to_string(), format!("{}-no-layout", runtime())];
    }
    let mut scopes = vec![
        runtime().to_string(),
        layout().to_string(),
        format!("{}-{}", runtime(), layout()),
    ];
    if cfg!(any(feature = "borsh", feature = "wincode")) {
        scopes.push("heap".to_string());
    }
    scopes
}

fn rust_files(dir: &Path) -> Vec<PathBuf> {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut files: Vec<PathBuf> = entries
        .map(|entry| entry.expect("readable directory entry").path())
        .filter(|path| path.extension().is_some_and(|ext| ext == "rs"))
        .collect();
    files.sort();
    files
}

fn fixtures(kind: &str) -> Vec<PathBuf> {
    let root = Path::new("tests").join(kind);
    let mut files = if runs_top_level() {
        rust_files(&root)
    } else {
        Vec::new()
    };
    for scope in scopes() {
        files.extend(rust_files(&root.join(scope)));
    }
    files
}

#[test]
fn macro_rejections_and_valid_counterparts() {
    let t = trybuild::TestCases::new();
    let fail = fixtures("fail");
    assert!(!fail.is_empty(), "no failing fixtures found under tests/fail");
    for fixture in fail {
        t.compile_fail(fixture);
    }
    for fixture in fixtures("pass") {
        t.pass(fixture);
    }
}
