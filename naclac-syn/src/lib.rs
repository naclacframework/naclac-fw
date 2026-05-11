pub mod types;
pub mod parser;
pub mod pda;
pub mod instruction;

use std::fs;
use std::path::PathBuf;
use types::NaclacProgram;

pub fn parse_workspace_program(program_dir: &PathBuf, program_name: &str, is_zero_copy: bool) -> NaclacProgram {
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
            for entry in entries {
                if let Ok(entry) = entry {
                    let path = entry.path();
                    if path.is_dir() {
                        dirs_to_visit.push(path);
                    } else if path.extension().unwrap_or_default() == "rs" {
                        rs_files.push(path);
                    }
                }
            }
        }
    }

    let mut all_codes = Vec::new();
    for file_path in &rs_files {
        if let Ok(code) = fs::read_to_string(file_path) {
            parser::parse_file(&mut idl, &code);
            all_codes.push(code);
        }
    }

    // Determine instruction order from lib.rs
    let mut func_order = Vec::new();
    let lib_path = program_dir.join("src/lib.rs");
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
    instruction::extract_instructions(&mut idl, &all_codes, &func_order);

    // Reorder instructions to match lib.rs module declaration
    let mut ordered_instructions = Vec::new();
    for target in &func_order {
        let camel_target = heck::ToLowerCamelCase::to_lower_camel_case(target.as_str());
        if let Some(pos) = idl.instructions.iter().position(|ix| ix.name == camel_target) {
            ordered_instructions.push(idl.instructions.remove(pos));
        }
    }
    idl.instructions = ordered_instructions;

    idl
}
