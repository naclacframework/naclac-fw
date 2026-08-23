use super::map_type_to_rust;
use crate::{Idl, IdlInstruction, IdlPda, IdlSeed};
use heck::{AsSnakeCase, ToUpperCamelCase};
use std::fs;
use std::path::Path;

pub fn generate_lib(
    idl: &Idl,
    clients_dir: &Path,
    header: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut lib_content = header.to_string();
    lib_content
        .push_str("#![cfg_attr(all(feature = \"cpi\", feature = \"pinocchio\"), no_std)]\n\n");
    // Two independent aliases, each resolved only by its own feature — never
    // by the absence of the other. `cpi` and `offchain` can both be active in
    // the same build (e.g. a downstream crate's own tests CPI-calling a
    // different program), and `naclac_client`/`naclac_lang::prelude` each
    // define their own, non-interchangeable `Address`/`Bool`/etc., so a
    // single shared alias name can't correctly serve both at once.
    lib_content
        .push_str("#[cfg(feature = \"cpi\")]\npub use naclac_lang::prelude as sdk_core_cpi;\n");
    lib_content.push_str(
        "#[cfg(feature = \"offchain\")]\npub use naclac_client as sdk_core_offchain;\n\n",
    );
    // `mod components`/`mod instructions` are only emitted when there's
    // actually something for them to declare — `naclac-client-gen/src/rust/mod.rs`
    // only creates the `src/components`/`src/instructions` directories (and
    // so only `naclac-client-gen/src/rust/components.rs`/`instructions.rs`
    // only ever write a `mod.rs` into them) when `idl.accounts`/
    // `idl.instructions` are non-empty — declaring the module unconditionally
    // here produced `E0583: file not found for module` for any program with
    // zero components (a pure-logic program, e.g. `tests/error-codes/`).
    if !idl.accounts.is_empty() {
        lib_content.push_str("#[cfg(feature = \"offchain\")]\n");
        lib_content.push_str("pub mod components;\n");
    }
    if !idl.instructions.is_empty() {
        lib_content.push_str("pub mod instructions;\n");
    }
    lib_content.push_str("pub mod types;\n\n");
    if !idl.accounts.is_empty() {
        lib_content.push_str("#[cfg(feature = \"offchain\")]\n");
        lib_content.push_str("pub use components::*;\n");
    }
    if !idl.instructions.is_empty() {
        lib_content.push_str("pub use instructions::*;\n");
    }
    lib_content.push_str("pub use types::*;\n\n");

    lib_content.push_str(&format!(
        "#[macro_export]\nmacro_rules! declare_id {{\n    ($id:expr) => {{}};\n}}\n\ndeclare_id!(\"{}\");\n\n",
        idl.address
    ));

    // PDA Helper Functions
    let mut account_pdas: std::collections::HashMap<String, Vec<(&IdlInstruction, &IdlPda)>> =
        std::collections::HashMap::new();
    for ix in &idl.instructions {
        for acc in &ix.accounts {
            if let Some(pda) = &acc.pda {
                account_pdas
                    .entry(acc.name.clone())
                    .or_default()
                    .push((ix, pda));
            }
        }
    }

    let mut sorted_acc_names: Vec<String> = account_pdas.keys().cloned().collect();
    sorted_acc_names.sort();

    for acc_name in sorted_acc_names {
        let pdas = &account_pdas[&acc_name];

        let (ix, pda) = pdas
            .iter()
            .max_by_key(|(ix, pda)| {
                let mut score: i64 = 0;
                if ix.name.starts_with("create") || ix.name.starts_with("init") {
                    score += 50;
                }
                let pda_count = ix.accounts.iter().filter(|a| a.pda.is_some()).count() as i64;
                score -= pda_count * 10;

                for seed in &pda.seeds {
                    match seed {
                        IdlSeed::Arg { path } => {
                            if ix.args.iter().any(|a| a.name == *path) {
                                score += 10;
                            } else {
                                score -= 5;
                            }
                        }
                        IdlSeed::Account { path, .. } => {
                            if *path == acc_name {
                                score -= 30;
                            } else if ix
                                .accounts
                                .iter()
                                .any(|a| a.name == *path && a.pda.is_none())
                            {
                                score += 5;
                            } else if ix.accounts.iter().any(|a| a.name == *path) {
                                score += 2;
                            }
                        }
                        _ => {}
                    }
                }
                score
            })
            .unwrap();

        // A `seeds::program = X` override means this PDA is derived under a
        // *different* program than the current one — the derivation program
        // is fixed at compile time, not something a caller should be able to
        // vary, so such a PDA takes no `program_id` param at all (unlike
        // every other helper here, which derives under the current program
        // and takes one). Only a `const` override is resolvable to a fixed
        // address; anything else (an arg/account-derived foreign program,
        // not expected in practice) is skipped like `unresolvable_field_seed`
        // below, rather than guessed at.
        let foreign_program_bytes: Option<&[u8]> = match &pda.program {
            None => None,
            Some(IdlSeed::Const { value, .. }) => Some(value.as_slice()),
            Some(_) => {
                lib_content.push_str(&format!(
                    "// get_{}_pda intentionally not generated: this PDA's `seeds::program`\n\
                     // override isn't a plain constant, so there's no fixed address to\n\
                     // derive against. Derive it manually.\n\n",
                    AsSnakeCase(&acc_name)
                ));
                continue;
            }
        };

        let mut params = if foreign_program_bytes.is_some() {
            Vec::new()
        } else {
            vec!["program_id: &naclac_client::Address".to_string()]
        };
        let mut local_defs = Vec::new();
        let mut seed_slices = Vec::new();
        let mut seen_params = std::collections::HashSet::new();
        // Set when a seed references a *field* of another account (e.g.
        // `registry.bump`) whose type naclac-syn couldn't resolve from the
        // backing component's struct definition (a nested/defined type, not
        // a plain primitive — see naclac_syn::pda::mk_account_seed). There is
        // no reliable way to generate correct code without knowing the real
        // type, so the helper is skipped entirely rather than guessed at.
        let mut unresolvable_field_seed: Option<String> = None;

        for seed in &pda.seeds {
            match seed {
                IdlSeed::Const { value, .. } => {
                    let bytes_str = value
                        .iter()
                        .map(|b| b.to_string())
                        .collect::<Vec<_>>()
                        .join(", ");
                    seed_slices.push(format!("&[{}]", bytes_str));
                }
                IdlSeed::Arg { path } => {
                    let param_name = AsSnakeCase(path).to_string();
                    let arg_ty = crate::resolve_leaf_type(path, &ix.args, &idl.defined_types)
                        .map(|ty| map_type_to_rust(&ty, idl.is_zero_copy))
                        .unwrap_or_else(|| "u64".to_string());

                    if !seen_params.contains(&param_name) {
                        seen_params.insert(param_name.clone());
                        params.push(format!("{}: {}", param_name, arg_ty));
                        if arg_ty == "crate::sdk_core_offchain::Address" {
                            // Direct
                        } else if arg_ty == "crate::sdk_core_offchain::String" || arg_ty == "&str" {
                            // Bytes
                        } else if arg_ty == "u8" {
                            local_defs
                                .push(format!("    let {}_arr = [{}];", param_name, param_name));
                        } else {
                            local_defs.push(format!(
                                "    let {}_bytes = {}.to_le_bytes();",
                                param_name, param_name
                            ));
                        }
                    }

                    if arg_ty == "crate::sdk_core_offchain::Address" {
                        seed_slices.push(format!("{}.as_ref()", param_name));
                    } else if arg_ty == "crate::sdk_core_offchain::String" || arg_ty == "&str" {
                        seed_slices.push(format!("{}.as_bytes()", param_name));
                    } else if arg_ty == "u8" {
                        seed_slices.push(format!("&{}_arr", param_name));
                    } else {
                        seed_slices.push(format!("&{}_bytes", param_name));
                    }
                }
                IdlSeed::Account { path, field_type } => {
                    if path.contains('.') {
                        let Some(field_ty) = field_type else {
                            unresolvable_field_seed = Some(path.clone());
                            break;
                        };
                        // Resolved field-access seed (e.g. `registry.bump: u8`).
                        // Reuse the exact same primitive-type-to-byte-conversion
                        // rules as the `IdlSeed::Arg` branch above, just typed
                        // as a bare parameter rather than an instruction arg.
                        let param_name = AsSnakeCase(path.replace('.', "_")).to_string();
                        let rust_ty = match field_ty.as_str() {
                            "publicKey" => "&naclac_client::Address",
                            "string" | "String" => "&str",
                            other => other,
                        };
                        if !seen_params.contains(&param_name) {
                            seen_params.insert(param_name.clone());
                            params.push(format!("{}: {}", param_name, rust_ty));
                            if rust_ty == "&naclac_client::Address" || rust_ty == "&str" {
                                // Direct — no intermediate local needed.
                            } else if rust_ty == "u8" {
                                local_defs.push(format!(
                                    "    let {}_arr = [{}];",
                                    param_name, param_name
                                ));
                            } else {
                                local_defs.push(format!(
                                    "    let {}_bytes = {}.to_le_bytes();",
                                    param_name, param_name
                                ));
                            }
                        }
                        if rust_ty == "&naclac_client::Address" {
                            seed_slices.push(format!("{}.as_ref()", param_name));
                        } else if rust_ty == "&str" {
                            seed_slices.push(format!("{}.as_bytes()", param_name));
                        } else if rust_ty == "u8" {
                            seed_slices.push(format!("&{}_arr", param_name));
                        } else {
                            seed_slices.push(format!("&{}_bytes", param_name));
                        }
                        continue;
                    }
                    let param_name = AsSnakeCase(path).to_string();
                    if param_name == AsSnakeCase(&acc_name).to_string() {
                        continue;
                    }
                    if !seen_params.contains(&param_name) {
                        seen_params.insert(param_name.clone());
                        params.push(format!("{}: &naclac_client::Address", param_name));
                    }
                    seed_slices.push(format!("{}.as_ref()", param_name));
                }
            }
        }

        if let Some(field_path) = unresolvable_field_seed {
            lib_content.push_str(&format!(
                "// get_{}_pda intentionally not generated: this PDA's seeds include\n\
                 // `{}`, a field read from another account whose type is not a plain\n\
                 // primitive (e.g. a nested/defined type) — naclac-syn couldn't resolve\n\
                 // it from the backing component's struct definition, so there's no\n\
                 // reliable way to generate correct code. Derive it manually — fetch the\n\
                 // account, read the field, and call Address::find_program_address\n\
                 // yourself with the correct byte conversion for that field's type.\n\n",
                AsSnakeCase(&acc_name),
                field_path
            ));
            continue;
        }

        let params_str = if params.is_empty() {
            String::new()
        } else {
            format!("\n    {}\n", params.join(",\n    "))
        };
        lib_content.push_str(&format!(
            "#[cfg(feature = \"offchain\")]\n\
             pub fn get_{}_pda({}) -> (naclac_client::Address, u8) {{\n",
            AsSnakeCase(&acc_name),
            params_str
        ));
        for def in &local_defs {
            lib_content.push_str(&format!("{}\n", def));
        }
        let derivation_program_expr = if let Some(bytes) = foreign_program_bytes {
            let bytes_str = bytes
                .iter()
                .map(|b| b.to_string())
                .collect::<Vec<_>>()
                .join(", ");
            lib_content.push_str(&format!(
                "    let __derivation_program = naclac_client::Address::new_from_array([{}]);\n",
                bytes_str
            ));
            "&__derivation_program".to_string()
        } else {
            "program_id".to_string()
        };
        lib_content.push_str("    naclac_client::Address::find_program_address(\n        &[\n");
        for slice in &seed_slices {
            lib_content.push_str(&format!("            {},\n", slice));
        }
        lib_content.push_str(&format!(
            "        ],\n        {},\n    )\n}}\n\n",
            derivation_program_expr
        ));
    }

    let program_camel = idl.metadata.name.to_upper_camel_case();
    let id_bytes = bs58::decode(&idl.address).into_vec().unwrap();
    let bytes_str = id_bytes
        .iter()
        .map(|b| b.to_string())
        .collect::<Vec<_>>()
        .join(", ");

    lib_content.push_str("#[cfg(feature = \"cpi\")]\n");
    lib_content.push_str(&format!(
        "#[derive(Clone, Copy)]\n\
         pub struct {};\n\n\
         #[cfg(feature = \"cpi\")]\n\
         impl sdk_core_cpi::Id for {} {{\n\
         \x20   fn id() -> sdk_core_cpi::Address {{\n\
         \x20       sdk_core_cpi::Address::new_from_array([{}])\n\
         \x20   }}\n\
         }}\n\n",
        program_camel, program_camel, bytes_str
    ));

    fs::write(clients_dir.join("src/lib.rs"), lib_content).unwrap();
    Ok(())
}
