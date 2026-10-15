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

use {
    crate::modules::{cfg_attrs, crate_modules, reaches_root, CrateModule},
    std::path::Path,
};

/// One `const`/`struct`/`enum` item found carrying the scanned-for
/// attribute.
pub struct TaggedItem {
    /// The path of the module declaring the item (e.g. `"crate::constants"`),
    /// as the crate's module tree defines it.
    pub module_path: String,
    pub name: String,
    /// True when an `#[event(..)]` attribute's arguments include `alloc`
    /// (alone or alongside `discriminator = ..`) — always `false` for any
    /// `attr_name` other than `"event"`.
    pub alloc: bool,
    /// The `#[cfg(..)]` attributes gating the item: its modules' then its own.
    /// Code that names the item must carry them, so it exists exactly when
    /// the item does.
    pub cfgs: Vec<syn::Attribute>,
    /// Whether `module_path::name` can be named from every module of the
    /// crate. When false, generated code naming it would not compile.
    pub visible_from_root: bool,
}

fn item_visible(module: &CrateModule, vis: &syn::Visibility) -> bool {
    module.visible_from_root && reaches_root(vis, module.depth)
}

fn item_cfgs(module: &CrateModule, attrs: &[syn::Attribute]) -> Vec<syn::Attribute> {
    let mut cfgs = module.cfgs.clone();
    cfgs.extend(cfg_attrs(attrs));
    cfgs
}

/// Every `const`/`struct`/`enum` in any module of the crate whose attribute
/// list contains `attr_name` (bare presence only, matching how
/// `#[constant]`/`#[event]`/`#[error_code]` are already detected elsewhere
/// in this crate — no argument parsing beyond checking for the `alloc`
/// marker on `#[event(..)]`).
pub fn scan_tagged_items(program_dir: &Path, attr_name: &str) -> Vec<TaggedItem> {
    let mut found = Vec::new();
    for module in crate_modules(program_dir) {
        for item in &module.items {
            let (attrs, ident, vis) = match item {
                syn::Item::Struct(s) => (&s.attrs, &s.ident, &s.vis),
                syn::Item::Enum(e) => (&e.attrs, &e.ident, &e.vis),
                syn::Item::Const(c) => (&c.attrs, &c.ident, &c.vis),
                _ => continue,
            };
            if let Some(attr) = attrs.iter().find(|a| a.path().is_ident(attr_name)) {
                let alloc = attr_name == "event"
                    && crate::discriminator::macro_args_of(attr, true).is_ok_and(|args| args.alloc);
                found.push(TaggedItem {
                    module_path: module.path.clone(),
                    name: ident.to_string(),
                    alloc,
                    cfgs: item_cfgs(&module, attrs),
                    visible_from_root: item_visible(&module, vis),
                });
            }
        }
    }
    found
}

/// A `pub const` item of the crate that carries no `#[constant]`, with the
/// path it can be named by from the crate root.
pub struct PubConst {
    pub module_path: String,
    pub name: String,
    pub ty: syn::Type,
    pub docs: Vec<String>,
    /// Same meaning as [`TaggedItem::cfgs`].
    pub cfgs: Vec<syn::Attribute>,
    /// Same meaning as [`TaggedItem::visible_from_root`].
    pub visible_from_root: bool,
}

/// Every `pub const` in any module of the crate without a `#[constant]`
/// attribute.
pub fn scan_untagged_pub_consts(program_dir: &Path) -> Vec<PubConst> {
    let mut found = Vec::new();
    for module in crate_modules(program_dir) {
        for item in &module.items {
            let syn::Item::Const(item_const) = item else {
                continue;
            };
            let is_pub = matches!(item_const.vis, syn::Visibility::Public(_));
            let tagged = item_const
                .attrs
                .iter()
                .any(|attr| attr.path().is_ident("constant"));
            if is_pub && !tagged {
                found.push(PubConst {
                    module_path: module.path.clone(),
                    name: item_const.ident.to_string(),
                    ty: (*item_const.ty).clone(),
                    docs: crate::parser::extract_docs(&item_const.attrs),
                    cfgs: item_cfgs(&module, &item_const.attrs),
                    visible_from_root: item_visible(&module, &item_const.vis),
                });
            }
        }
    }
    found
}
