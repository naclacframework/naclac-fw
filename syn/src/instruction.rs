//! Source-text helpers for the `#[program]` handlers' doc comments and for
//! constants that hold an address.

use {
    crate::modules::CrateModule,
    std::path::Path,
    syn::{Item, ItemFn},
};

/// Resolves a constant's raw IDL `value` string down to a plain base58
/// address string. Handles a plain string literal (`"..."`) and a
/// `address!("...")`/`pubkey!("...")` macro-call literal (the raw token
/// text stored for any non-string-literal constant expression) — without
/// this, a constant declared as `pub const X: Address = address!("...")`
/// would leak the literal macro-call source text as the resolved address.
pub(crate) fn resolve_constant_address_str(val: &str) -> String {
    if val.starts_with('"') && val.ends_with('"') {
        return val[1..val.len() - 1].to_string();
    }
    for macro_name in ["address!", "pubkey!"] {
        if let Some(rest) = val.strip_prefix(macro_name) {
            let rest = rest.trim_start_matches('(').trim_end_matches(')');
            if rest.starts_with('"') && rest.ends_with('"') {
                return rest[1..rest.len() - 1].to_string();
            }
        }
    }
    val.to_string()
}

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
