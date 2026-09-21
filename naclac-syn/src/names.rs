//! Targeted name-and-module-path-only scanning for `idl-build`. Unlike
//! `parse_workspace_program` (which resolves full type/field shapes via
//! AST, exactly what `idl-build` exists to stop relying on for real
//! values), this only answers "which `const`/`struct`/`enum` items in this
//! crate carry a given attribute, and where do they live" — a structural
//! question a real compiled program cannot answer on its own, since an
//! item carrying an attribute macro that doesn't itself register anything
//! discoverable (constants, events, errors — see
//! docs/plan/idl-build-compilation-migration.md) produces no trace
//! `#[program]` could otherwise find. Used only at the crate's own build
//! time, gated on `CARGO_FEATURE_IDL_BUILD` being set — never during a
//! normal build, and never to read a value or a type shape, only a name
//! and the path needed to call that item's own generated print function.

use std::fs;
use std::path::Path;

/// One `const`/`struct`/`enum` item found carrying the scanned-for
/// attribute — `module_path` is that item's real path from crate root
/// (e.g. `"crate::constants"`), assuming the standard Rust file-to-module
/// convention (`src/foo/bar.rs` → `crate::foo::bar`, `mod.rs`/`lib.rs`/
/// `main.rs` collapse to their parent) — not resolved for a `#[path = ...]`
/// override, the one real gap in this scan.
pub struct TaggedItem {
    pub module_path: String,
    pub name: String,
    /// True when the attribute's argument is exactly the bare `alloc`
    /// identifier (i.e. `#[event(alloc)]`) — meaningless for any
    /// `attr_name` other than `"event"`, always `false` there since
    /// `#[constant]`/`#[error_code]` take no arguments.
    pub alloc: bool,
}

/// Every top-level `const`/`struct`/`enum` in `program_dir/src/**/*.rs`
/// whose attribute list contains `attr_name` (bare presence only, matching
/// how `#[constant]`/`#[event]`/`#[error_code]` are already detected
/// elsewhere in this crate — no argument parsing beyond checking for the
/// `alloc` marker on `#[event(alloc)]`).
pub fn scan_tagged_items(program_dir: &Path, attr_name: &str) -> Vec<TaggedItem> {
    let mut found = Vec::new();
    let mut dirs_to_visit = vec![program_dir.join("src")];

    while let Some(dir) = dirs_to_visit.pop() {
        let Ok(entries) = fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                dirs_to_visit.push(path);
                continue;
            }
            if path.extension().unwrap_or_default() != "rs" {
                continue;
            }
            let Ok(code) = fs::read_to_string(&path) else {
                continue;
            };
            let Ok(file) = syn::parse_file(&code) else {
                continue;
            };
            let module_path = file_to_module_path(program_dir, &path);
            for item in &file.items {
                let (attrs, ident) = match item {
                    syn::Item::Struct(s) => (&s.attrs, &s.ident),
                    syn::Item::Enum(e) => (&e.attrs, &e.ident),
                    syn::Item::Const(c) => (&c.attrs, &c.ident),
                    _ => continue,
                };
                if let Some(attr) = attrs.iter().find(|a| a.path().is_ident(attr_name)) {
                    let alloc = attr
                        .parse_args::<syn::Ident>()
                        .is_ok_and(|ident| ident == "alloc");
                    found.push(TaggedItem {
                        module_path: module_path.clone(),
                        name: ident.to_string(),
                        alloc,
                    });
                }
            }
        }
    }

    found
}

fn file_to_module_path(program_dir: &Path, file_path: &Path) -> String {
    let src_dir = program_dir.join("src");
    let rel = file_path.strip_prefix(&src_dir).unwrap_or(file_path);

    let mut components: Vec<String> = rel
        .components()
        .map(|c| c.as_os_str().to_string_lossy().into_owned())
        .collect();

    if let Some(last) = components.last_mut() {
        if let Some(stripped) = last.strip_suffix(".rs") {
            *last = stripped.to_string();
        }
    }
    match components.last().map(String::as_str) {
        Some("lib") | Some("main") | Some("mod") => {
            components.pop();
        }
        _ => {}
    }

    if components.is_empty() {
        "crate".to_string()
    } else {
        format!("crate::{}", components.join("::"))
    }
}
