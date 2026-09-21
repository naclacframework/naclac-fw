pub mod discriminator;
pub mod instruction;
pub mod names;
pub mod parser;
pub mod pda;
pub mod types;

use std::fs;
use std::path::Path;
use types::NaclacProgram;

pub fn parse_workspace_program(
    program_dir: &Path,
    program_name: &str,
    is_zero_copy: bool,
) -> NaclacProgram {
    let mut idl = NaclacProgram {
        name: program_name.to_string(),
        is_zero_copy,
        instructions: Vec::new(),
        accounts: Vec::new(),
        events: Vec::new(),
        errors: Vec::new(),
        constants: Vec::new(),
        types: Vec::new(),
    };

    // Parse base files for components, events, errors, constants
    let mut rs_files = Vec::new();
    let mut dirs_to_visit = vec![program_dir.join("src")];

    while let Some(dir) = dirs_to_visit.pop() {
        if let Ok(entries) = fs::read_dir(&dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    dirs_to_visit.push(path);
                } else if path.extension().unwrap_or_default() == "rs" {
                    rs_files.push(path);
                }
            }
        }
    }

    let lib_path = program_dir.join("src/lib.rs");

    // Pass 1: collect every constant across the whole crate first. Array-length
    // resolution and other constant lookups during pass 2 (`parser::parse_file`)
    // need the complete, whole-crate constant set — not just whatever this one
    // file happens to declare — regardless of which order `rs_files` (an
    // unordered directory walk) happens to visit files in.
    let mut all_codes = Vec::new();
    let mut is_lib_rs_flags = Vec::new();
    for file_path in &rs_files {
        if let Ok(code) = fs::read_to_string(file_path) {
            parser::parse_constants_pass(&mut idl, &code);
            is_lib_rs_flags.push(file_path == &lib_path);
            all_codes.push(code);
        }
    }

    // Pass 2: everything else, now that idl.constants is complete.
    for code in &all_codes {
        parser::parse_file(&mut idl, code);
    }

    // Determine instruction order from lib.rs
    let mut func_order = Vec::new();
    if let Ok(lib_code) = fs::read_to_string(&lib_path) {
        if let Ok(lib_tree) = syn::parse_file(&lib_code) {
            for item in lib_tree.items {
                if let syn::Item::Mod(item_mod) = item {
                    if let Some((_, items)) = item_mod.content {
                        for inner_item in items {
                            if let syn::Item::Fn(item_fn) = inner_item {
                                func_order.push(item_fn.sig.ident.to_string());
                            }
                        }
                    }
                }
            }
        }
    }

    // Parse Instructions in 2 passes
    instruction::extract_instructions(&mut idl, &all_codes, &is_lib_rs_flags, &func_order);

    // Reorder instructions to match lib.rs module declaration
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
