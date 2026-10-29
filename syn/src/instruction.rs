//! Source-text lookup of the `#[program]` handlers' doc comments.

use {
    crate::modules::CrateModule,
    std::path::Path,
    syn::{Item, ItemFn},
};

fn module_fns(module: &CrateModule) -> impl Iterator<Item = &ItemFn> {
    module.items.iter().filter_map(|item| match item {
        Item::Fn(item_fn) => Some(item_fn),
        _ => None,
    })
}

/// The doc comment shown for handler `func_name`: the one on a function of
/// that name written in the crate root file (its `#[program]` wrapper) when
/// that has one, otherwise the first documented function of the same name in
/// module order.
pub fn resolve_instruction_docs(
    modules: &[CrateModule],
    crate_root: &Path,
    func_name: &str,
) -> Vec<String> {
    let mut docs = Vec::new();
    for module in modules {
        let in_root_file = module.file == crate_root;
        for item_fn in module_fns(module) {
            if item_fn.sig.ident != func_name {
                continue;
            }
            let candidate = crate::parser::extract_docs(&item_fn.attrs);
            if !candidate.is_empty() && (in_root_file || docs.is_empty()) {
                docs = candidate;
            }
        }
    }
    docs
}
