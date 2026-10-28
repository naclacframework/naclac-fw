//! The crate's module tree as rustc builds it from the crate root: which files
//! belong to the crate, which module each item lives in, and which `#[cfg]`
//! attributes gate it. Every naclac tool that reads a program's source takes
//! its items from here, so the compiled `idl-build` path and the macros agree
//! on what the crate declares and where.

use std::{
    fs,
    path::{Path, PathBuf},
};

/// The crate root Cargo uses when `Cargo.toml` sets no `[lib] path`.
const DEFAULT_CRATE_ROOT: &str = "src/lib.rs";

/// One module of the crate.
#[derive(Clone)]
pub struct CrateModule {
    /// `crate`, `crate::a`, `crate::a::b`, ...
    pub path: String,
    /// The source file the module's items are written in (for an inline
    /// module, the file containing the `mod name { .. }` block).
    pub file: PathBuf,
    /// Items declared directly in this module. A child module is present only
    /// as its `mod` declaration with no body; its items are in its own
    /// `CrateModule`.
    pub items: Vec<syn::Item>,
    /// The `#[cfg(..)]` attributes on this module's declaration and on every
    /// enclosing module's, outermost first. Empty for the crate root.
    pub cfgs: Vec<syn::Attribute>,
    /// Nesting depth: 0 for the crate root, 1 for its direct children, ...
    pub depth: usize,
    /// Whether code anywhere in the crate can name this module by its path,
    /// i.e. every module declaration on the way down is visible at the root.
    pub visible_from_root: bool,
}

/// The root file of the crate cargo is compiling: that of the binary named by
/// `CARGO_BIN_NAME` when cargo sets it, otherwise the library's. See
/// [`crate_root_of`].
pub fn crate_root(program_dir: &Path) -> PathBuf {
    crate_root_of(program_dir, std::env::var("CARGO_BIN_NAME").ok().as_deref())
}

/// The crate root file of the library (`[lib] path` from `Cargo.toml` when
/// set, otherwise Cargo's default `src/lib.rs`), or, when `bin` names the
/// binary being compiled, of that binary: the `path` of its `[[bin]]` entry.
///
/// Panics, naming the file and the cause, when `Cargo.toml` cannot be read, is
/// not valid TOML, has a `lib` or `lib.path` of the wrong type, or lacks a
/// `[[bin]]` entry with a string `path` for `bin`: the module tree cannot be
/// read without knowing the crate root.
pub fn crate_root_of(program_dir: &Path, bin: Option<&str>) -> PathBuf {
    let manifest_path = program_dir.join("Cargo.toml");
    let text = fs::read_to_string(&manifest_path)
        .unwrap_or_else(|e| panic!("naclac-syn: cannot read {}: {e}", manifest_path.display()));
    let manifest: toml::Value = toml::from_str(&text).unwrap_or_else(|e| {
        panic!(
            "naclac-syn: {} is not valid TOML: {e}",
            manifest_path.display()
        )
    });
    if let Some(bin) = bin {
        let path = manifest
            .get("bin")
            .and_then(toml::Value::as_array)
            .and_then(|bins| {
                bins.iter()
                    .find(|entry| entry.get("name").and_then(toml::Value::as_str) == Some(bin))
            })
            .and_then(|entry| entry.get("path"))
            .and_then(toml::Value::as_str)
            .unwrap_or_else(|| {
                panic!(
                    "naclac-syn: the binary `{bin}` being compiled has no `[[bin]]` entry with a \
                     `path` in {}",
                    manifest_path.display()
                )
            });
        return program_dir.join(path);
    }
    let lib_path = match manifest.get("lib") {
        None => DEFAULT_CRATE_ROOT,
        Some(toml::Value::Table(lib)) => match lib.get("path") {
            None => DEFAULT_CRATE_ROOT,
            Some(toml::Value::String(path)) => path.as_str(),
            Some(_) => panic!(
                "naclac-syn: `path` under `[lib]` in {} must be a string",
                manifest_path.display()
            ),
        },
        Some(_) => panic!(
            "naclac-syn: `lib` in {} must be a table",
            manifest_path.display()
        ),
    };
    program_dir.join(lib_path)
}

/// Every module of the crate, parents before children, in declaration
/// order. Panics when the crate root file cannot be read or does not parse.
/// A child module whose file is missing or does not parse contributes
/// nothing: a `#[cfg]`-disabled `mod x;` has no file rustc loads, and a file
/// that is needed but broken fails the compilation of the crate itself.
pub fn crate_modules(program_dir: &Path) -> Vec<CrateModule> {
    let root = crate_root(program_dir);
    let mut out = Vec::new();
    let text = fs::read_to_string(&root).unwrap_or_else(|e| {
        panic!(
            "naclac-syn: cannot read the crate root {}: {e}",
            root.display()
        )
    });
    let file = syn::parse_file(&text).unwrap_or_else(|e| {
        panic!(
            "naclac-syn: the crate root {} does not parse: {e}",
            root.display()
        )
    });
    let children_dir = root.parent().map(Path::to_path_buf).unwrap_or_default();
    walk(
        &mut out,
        Job {
            path: "crate".to_string(),
            file: root,
            items: file.items,
            children_dir,
            in_inline: false,
            cfgs: Vec::new(),
            depth: 0,
            visible_from_root: true,
        },
    );
    out
}

/// Whether an item or module declared with `vis` inside a module at
/// `declared_in_depth` can be named from every module of the crate. Rust
/// makes a declaration visible in its own module and that module's
/// descendants; `pub(super)` lifts it one level, `pub(crate)`/`pub` to the
/// whole crate.
pub fn reaches_root(vis: &syn::Visibility, declared_in_depth: usize) -> bool {
    match vis {
        syn::Visibility::Public(_) => true,
        syn::Visibility::Inherited => declared_in_depth == 0,
        syn::Visibility::Restricted(restricted) => {
            let path = &restricted.path;
            if path.is_ident("crate") {
                true
            } else if path.is_ident("super") {
                declared_in_depth <= 1
            } else {
                declared_in_depth == 0
            }
        }
    }
}

struct Job {
    path: String,
    file: PathBuf,
    items: Vec<syn::Item>,
    children_dir: PathBuf,
    in_inline: bool,
    cfgs: Vec<syn::Attribute>,
    depth: usize,
    visible_from_root: bool,
}

fn parse(file: &Path) -> Option<syn::File> {
    syn::parse_file(&fs::read_to_string(file).ok()?).ok()
}

fn walk(out: &mut Vec<CrateModule>, job: Job) {
    let mut own_items = Vec::with_capacity(job.items.len());
    let mut children = Vec::new();

    for item in job.items {
        let syn::Item::Mod(mut item_mod) = item else {
            own_items.push(item);
            continue;
        };
        let name = item_mod.ident.to_string();
        let mut cfgs = job.cfgs.clone();
        cfgs.extend(cfg_attrs(&item_mod.attrs));
        let path_attr = path_attr(&item_mod.attrs);
        let child_path = format!("{}::{name}", job.path);
        let depth = job.depth + 1;
        let visible_from_root = job.visible_from_root && reaches_root(&item_mod.vis, job.depth);

        match item_mod.content.take() {
            Some((_, inner)) => children.push(Job {
                path: child_path,
                file: job.file.clone(),
                items: inner,
                children_dir: job.children_dir.join(path_attr.as_deref().unwrap_or(&name)),
                in_inline: true,
                cfgs,
                depth,
                visible_from_root,
            }),
            None => {
                let (file, children_dir) = match path_attr {
                    Some(relative) => {
                        let base = if job.in_inline {
                            job.children_dir.clone()
                        } else {
                            job.file.parent().map(Path::to_path_buf).unwrap_or_default()
                        };
                        let file = base.join(relative);
                        let dir = file.parent().map(Path::to_path_buf).unwrap_or_default();
                        (file, dir)
                    }
                    None => {
                        let flat = job.children_dir.join(format!("{name}.rs"));
                        let file = if flat.exists() {
                            flat
                        } else {
                            job.children_dir.join(&name).join("mod.rs")
                        };
                        (file, job.children_dir.join(&name))
                    }
                };
                if let Some(parsed) = parse(&file) {
                    children.push(Job {
                        path: child_path,
                        file,
                        items: parsed.items,
                        children_dir,
                        in_inline: false,
                        cfgs,
                        depth,
                        visible_from_root,
                    });
                }
            }
        }
        own_items.push(syn::Item::Mod(item_mod));
    }

    out.push(CrateModule {
        path: job.path,
        file: job.file,
        items: own_items,
        cfgs: job.cfgs,
        depth: job.depth,
        visible_from_root: job.visible_from_root,
    });
    for child in children {
        walk(out, child);
    }
}

fn path_attr(attrs: &[syn::Attribute]) -> Option<String> {
    attrs.iter().find_map(|attr| {
        if !attr.path().is_ident("path") {
            return None;
        }
        let syn::Meta::NameValue(nv) = &attr.meta else {
            return None;
        };
        let syn::Expr::Lit(syn::ExprLit {
            lit: syn::Lit::Str(s),
            ..
        }) = &nv.value
        else {
            return None;
        };
        Some(s.value())
    })
}

/// The `#[cfg(..)]` attributes in `attrs`.
pub fn cfg_attrs(attrs: &[syn::Attribute]) -> Vec<syn::Attribute> {
    attrs
        .iter()
        .filter(|attr| attr.path().is_ident("cfg"))
        .cloned()
        .collect()
}

/// The attributes of an item of any kind that carries them.
pub fn item_attrs(item: &syn::Item) -> &[syn::Attribute] {
    match item {
        syn::Item::Const(i) => &i.attrs,
        syn::Item::Enum(i) => &i.attrs,
        syn::Item::ExternCrate(i) => &i.attrs,
        syn::Item::Fn(i) => &i.attrs,
        syn::Item::ForeignMod(i) => &i.attrs,
        syn::Item::Impl(i) => &i.attrs,
        syn::Item::Macro(i) => &i.attrs,
        syn::Item::Mod(i) => &i.attrs,
        syn::Item::Static(i) => &i.attrs,
        syn::Item::Struct(i) => &i.attrs,
        syn::Item::Trait(i) => &i.attrs,
        syn::Item::TraitAlias(i) => &i.attrs,
        syn::Item::Type(i) => &i.attrs,
        syn::Item::Union(i) => &i.attrs,
        syn::Item::Use(i) => &i.attrs,
        _ => &[],
    }
}

/// Whether every `#[cfg(..)]` in `attrs` is built only from `feature = ".."`,
/// `test`, `any`, `all` and `not`, the predicates whose outcome does not
/// depend on the target the program is compiled for.
pub fn cfg_is_decidable(attrs: &[syn::Attribute]) -> bool {
    attrs
        .iter()
        .filter(|attr| attr.path().is_ident("cfg"))
        .all(|attr| {
            attr.parse_args::<syn::Meta>()
                .is_ok_and(|meta| decidable(&meta))
        })
}

fn decidable(meta: &syn::Meta) -> bool {
    match meta {
        syn::Meta::NameValue(nv) => nv.path.is_ident("feature"),
        syn::Meta::Path(path) => path.is_ident("test"),
        syn::Meta::List(list) => {
            (list.path.is_ident("any") || list.path.is_ident("all") || list.path.is_ident("not"))
                && list
                    .parse_args_with(
                        syn::punctuated::Punctuated::<syn::Meta, syn::Token![,]>::parse_terminated,
                    )
                    .is_ok_and(|nested| nested.iter().all(decidable))
        }
    }
}
