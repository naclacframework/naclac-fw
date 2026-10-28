pub mod aliases;
pub mod bounds;
pub mod const_eval;
pub mod discriminator;
pub mod idl_items;
pub mod instruction;
pub mod modules;
pub mod names;
pub mod opt_type;
pub mod parser;
pub mod pda;
pub mod program_id;
pub mod types;
pub mod zc_codegen;

use std::path::Path;

/// Doc comments of the named `#[program]` handlers, resolved from the crate's
/// modules (the crate-root wrapper's docs win, else the implementation's).
/// Handlers without docs map to an empty list.
pub fn instruction_docs(
    program_dir: &Path,
    names: &[String],
) -> std::collections::HashMap<String, Vec<String>> {
    let modules = modules::crate_modules(program_dir);
    let root = modules::crate_root(program_dir);
    names
        .iter()
        .map(|name| {
            (
                name.clone(),
                instruction::resolve_instruction_docs(&modules, &root, name),
            )
        })
        .collect()
}

/// The constants collected from the crate's modules: every `pub const` and
/// every `#[constant]`. Callers that resolve a `[T; N]` length or another
/// constant reference must use this set, so they agree on what `N` is.
pub fn crate_constants(program_dir: &Path) -> Vec<types::NaclacConstant> {
    let mut constants = Vec::new();
    let mut crate_modules = modules::crate_modules(program_dir);
    aliases::Aliases::collect(&crate_modules).resolve_modules(&mut crate_modules);
    let mut env = const_eval::ConstEnv::default();
    for module in &crate_modules {
        env.collect(&module.items);
    }
    for module in &crate_modules {
        parser::parse_constants_pass(&mut constants, &module.items, &env);
    }
    constants
}
