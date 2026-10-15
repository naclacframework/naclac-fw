//! The crate's module tree as rustc builds it from the crate root: which files
//! belong to the crate, which module each item lives in, and which `#[cfg]`
//! attributes gate it. Every naclac tool that reads a program's source takes
//! its items from here, so the AST IDL generator, the compiled `idl-build`
//! path and the macros agree on what the crate declares and where.

use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};

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

/// The crate root file: `[lib] path` from `Cargo.toml` when set, otherwise
/// `src/lib.rs`.
pub fn crate_root(program_dir: &Path) -> PathBuf {
    let lib_path = fs::read_to_string(program_dir.join("Cargo.toml"))
        .ok()
        .and_then(|content| toml::from_str::<toml::Value>(&content).ok())
        .and_then(|manifest| Some(manifest.get("lib")?.get("path")?.as_str()?.to_string()))
        .unwrap_or_else(|| "src/lib.rs".to_string());
    program_dir.join(lib_path)
}

/// Every module of the crate, parents before children, in declaration
/// order. A module whose file is missing or does not parse contributes
/// nothing (the crate itself would not compile).
pub fn crate_modules(program_dir: &Path) -> Vec<CrateModule> {
    let root = crate_root(program_dir);
    let mut out = Vec::new();
    let Some(file) = parse(&root) else {
        return out;
    };
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

/// The modules and items the crate has when built with `features`, for a
/// reader with no compiler. A `#[cfg]` is evaluated through
/// [`cfg_active`]; modules and items it rules out are dropped.
pub fn active_crate_modules(program_dir: &Path, features: &HashSet<String>) -> Vec<CrateModule> {
    crate_modules(program_dir)
        .into_iter()
        .filter(|module| cfg_active(&module.cfgs, features))
        .map(|mut module| {
            module
                .items
                .retain(|item| cfg_active(item_attrs(item), features));
            module
        })
        .collect()
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

/// Paths of the crate-root modules carrying `#[program]`: the modules whose
/// `fn` items are the program's instruction handlers.
pub fn program_module_paths(modules: &[CrateModule]) -> Vec<String> {
    let Some(root) = modules.iter().find(|m| m.path == "crate") else {
        return Vec::new();
    };
    root.items
        .iter()
        .filter_map(|item| match item {
            syn::Item::Mod(item_mod)
                if item_mod.attrs.iter().any(|a| a.path().is_ident("program")) =>
            {
                Some(format!("crate::{}", item_mod.ident))
            }
            _ => None,
        })
        .collect()
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

/// The crate's own features enabled by default: `default` from `Cargo.toml`
/// `[features]`, closed over the plain feature names each enabled feature
/// lists. Entries naming a dependency (`dep:x`, `x/feature`, `x?/feature`)
/// are not features of this crate and are skipped.
pub fn default_features(program_dir: &Path) -> HashSet<String> {
    let table: HashMap<String, Vec<String>> = fs::read_to_string(program_dir.join("Cargo.toml"))
        .ok()
        .and_then(|content| toml::from_str::<toml::Value>(&content).ok())
        .and_then(|manifest| manifest.get("features")?.as_table().cloned())
        .map(|features| {
            features
                .into_iter()
                .map(|(name, list)| {
                    let entries = list
                        .as_array()
                        .map(|a| {
                            a.iter()
                                .filter_map(|v| v.as_str().map(str::to_string))
                                .collect()
                        })
                        .unwrap_or_default();
                    (name, entries)
                })
                .collect()
        })
        .unwrap_or_default();

    let mut enabled = HashSet::new();
    let mut pending = vec!["default".to_string()];
    while let Some(feature) = pending.pop() {
        for entry in table.get(&feature).into_iter().flatten() {
            if entry.starts_with("dep:") || entry.contains('/') {
                continue;
            }
            if enabled.insert(entry.clone()) {
                pending.push(entry.clone());
            }
        }
    }
    enabled
}

/// Whether every `#[cfg(..)]` in `attrs` holds for a build with exactly
/// `features` enabled. `feature = ".."`, `test` (false: no IDL describes a
/// test build), `any`, `all` and `not` are decidable here; a `#[cfg]` whose
/// outcome depends on any other predicate (`target_os`, ...) counts as
/// holding, so the item is kept rather than silently dropped.
pub fn cfg_active(attrs: &[syn::Attribute], features: &HashSet<String>) -> bool {
    attrs
        .iter()
        .filter(|attr| attr.path().is_ident("cfg"))
        .all(|attr| {
            attr.parse_args::<syn::Meta>()
                .ok()
                .and_then(|meta| eval_cfg(&meta, features))
                .unwrap_or(true)
        })
}

/// Whether every `#[cfg(..)]` in `attrs` is built only from `feature = ".."`,
/// `test`, `any`, `all` and `not`, so that [`cfg_active`] decides it the
/// same way the compiler does for the program's IDL build.
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

fn eval_cfg(meta: &syn::Meta, features: &HashSet<String>) -> Option<bool> {
    match meta {
        syn::Meta::Path(path) if path.is_ident("test") => Some(false),
        syn::Meta::NameValue(nv) if nv.path.is_ident("feature") => match &nv.value {
            syn::Expr::Lit(syn::ExprLit {
                lit: syn::Lit::Str(s),
                ..
            }) => Some(features.contains(&s.value())),
            _ => None,
        },
        syn::Meta::List(list) => {
            let nested = list
                .parse_args_with(
                    syn::punctuated::Punctuated::<syn::Meta, syn::Token![,]>::parse_terminated,
                )
                .ok()?;
            let values: Vec<Option<bool>> = nested.iter().map(|m| eval_cfg(m, features)).collect();
            if list.path.is_ident("not") {
                return match values.as_slice() {
                    [single] => single.map(|v| !v),
                    _ => None,
                };
            }
            if list.path.is_ident("all") {
                if values.contains(&Some(false)) {
                    return Some(false);
                }
                return values.iter().all(Option::is_some).then_some(true);
            }
            if list.path.is_ident("any") {
                if values.contains(&Some(true)) {
                    return Some(true);
                }
                return values.iter().all(Option::is_some).then_some(false);
            }
            None
        }
        _ => None,
    }
}
