//! The program address a crate declares with `declare_id!("<address>")` in
//! its crate root.

use {
    crate::modules::crate_modules,
    std::path::{Path, PathBuf},
    syn::Item,
};

/// The address a crate declares, and the file declaring it.
pub struct DeclaredId {
    pub address: String,
    pub file: PathBuf,
}

/// The address the crate in `program_dir` declares with
/// `declare_id!("<address>")` in its crate root. Errors, with a message naming
/// the crate root, when there is no such declaration, when there is more than
/// one, or when its argument is not a string literal.
pub fn declared_id(program_dir: &Path) -> Result<DeclaredId, String> {
    let modules = crate_modules(program_dir);
    let root = modules
        .iter()
        .find(|module| module.path == "crate")
        .ok_or_else(|| "the crate has no root module".to_string())?;

    let mut addresses = Vec::new();
    for item in &root.items {
        let Item::Macro(item_macro) = item else {
            continue;
        };
        let is_declare_id = item_macro
            .mac
            .path
            .segments
            .last()
            .is_some_and(|segment| segment.ident == "declare_id");
        if !is_declare_id {
            continue;
        }
        let literal: syn::LitStr = item_macro.mac.parse_body().map_err(|_| {
            format!(
                "`declare_id!` in {} must take the address as a string literal: \
                 `declare_id!(\"<address>\")`",
                root.file.display()
            )
        })?;
        addresses.push(literal.value());
    }

    match addresses.as_slice() {
        [address] => Ok(DeclaredId {
            address: address.clone(),
            file: root.file.clone(),
        }),
        [] => Err(format!(
            "the program must declare its address with `declare_id!(\"<address>\")` in its crate \
             root ({}); without it there is no program address",
            root.file.display()
        )),
        _ => Err(format!(
            "`declare_id!` appears more than once in {}; a program declares its address exactly \
             once",
            root.file.display()
        )),
    }
}
