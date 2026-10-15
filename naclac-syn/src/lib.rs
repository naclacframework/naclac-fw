pub mod const_eval;
pub mod discriminator;
pub mod idl_items;
pub mod instruction;
pub mod modules;
pub mod names;
pub mod opt_type;
pub mod parser;
pub mod pda;
pub mod types;

use std::path::Path;
use types::NaclacProgram;

/// Doc comments of the named `#[program]` handlers, resolved from the crate's
/// modules exactly as `parse_workspace_program` resolves them for the IDL
/// (the crate-root wrapper's docs win, else the implementation's). Handlers
/// without docs map to an empty list.
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

fn empty_program(name: &str) -> NaclacProgram {
    NaclacProgram {
        name: name.to_string(),
        instructions: Vec::new(),
        accounts: Vec::new(),
        events: Vec::new(),
        errors: Vec::new(),
        constants: Vec::new(),
        types: Vec::new(),
        event_cpi: false,
    }
}

/// The constants the IDL generator's first pass collects from the crate's
/// modules: every `pub const` and every `#[constant]`. Callers that resolve a
/// `[T; N]` length or another constant reference must use this set, so they
/// agree with the IDL generator on what `N` is.
pub fn crate_constants(program_dir: &Path) -> Vec<types::NaclacConstant> {
    let mut idl = empty_program("");
    let crate_modules = modules::crate_modules(program_dir);
    let mut env = const_eval::ConstEnv::default();
    for module in &crate_modules {
        env.collect(&module.items);
    }
    for module in &crate_modules {
        parser::parse_constants_pass(&mut idl, &module.items, &env, false);
    }
    idl.constants
}

/// The handlers of the crate-root module carrying `#[program]`, in
/// declaration order, with their `#[discriminator = ..]` overrides.
fn program_handlers(
    modules: &[modules::CrateModule],
) -> (Vec<String>, std::collections::HashMap<String, Vec<u8>>) {
    let mut func_order = Vec::new();
    let mut discriminator_overrides = std::collections::HashMap::new();
    let program_paths = modules::program_module_paths(modules);
    for module in modules.iter().filter(|m| program_paths.contains(&m.path)) {
        for item in &module.items {
            let syn::Item::Fn(item_fn) = item else {
                continue;
            };
            let fn_name = item_fn.sig.ident.to_string();
            match discriminator::parse_handler_override(&item_fn.attrs) {
                Ok(Some(bytes)) => {
                    discriminator_overrides.insert(fn_name.clone(), bytes);
                }
                Ok(None) => {}
                Err(e) => panic!("naclac-syn: invalid #[discriminator] on `{fn_name}`: {e}"),
            }
            func_order.push(fn_name);
        }
    }
    (func_order, discriminator_overrides)
}

/// The program's IDL content, read from its source. It does not depend on the
/// backend the program is built for (raw zero-copy, Pinocchio, Borsh or
/// Wincode): the same source gives the same result.
pub fn parse_workspace_program(program_dir: &Path, program_name: &str) -> NaclacProgram {
    let mut idl = empty_program(program_name);
    let features = modules::default_features(program_dir);
    let crate_modules = modules::active_crate_modules(program_dir, &features);
    let root = modules::crate_root(program_dir);

    let declarations = idl_items::declarations(&crate_modules)
        .unwrap_or_else(|e| panic!("naclac-syn: invalid `#[error_code]` attribute: {e}"));
    let violations = idl_items::violations(&declarations);
    if !violations.is_empty() {
        let messages: Vec<String> = violations.into_iter().map(|v| v.message).collect();
        panic!("{}", messages.join("\n"));
    }

    // Constants are complete before any other item is read: array lengths
    // resolved in pass 2 may refer to a constant declared in any module.
    let mut env = const_eval::ConstEnv::default();
    for module in &crate_modules {
        env.collect(&module.items);
    }
    for module in &crate_modules {
        parser::parse_constants_pass(&mut idl, &module.items, &env, true);
    }
    for module in &crate_modules {
        parser::parse_module_items(&mut idl, &module.items);
    }

    let (func_order, discriminator_overrides) = program_handlers(&crate_modules);
    instruction::extract_instructions(
        &mut idl,
        &crate_modules,
        &root,
        &func_order,
        &discriminator_overrides,
    );

    let mut ordered_instructions = Vec::new();
    for target in &func_order {
        if let Some(pos) = idl.instructions.iter().position(|ix| ix.name == *target) {
            ordered_instructions.push(idl.instructions.remove(pos));
        }
    }
    idl.instructions = ordered_instructions;

    parser::filter_unreachable_types(&mut idl);

    idl
}
