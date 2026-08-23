use crate::{Idl, IdlInstruction};
use heck::{AsSnakeCase, ToUpperCamelCase};

pub fn generate_offchain_ix(ix: &IdlInstruction, idl: &Idl) -> String {
    let mut offchain_content = String::new();
    let ix_snake = AsSnakeCase(&ix.name).to_string();
    let ix_camel = ix.name.to_upper_camel_case();

    // Accounts struct for off-chain. `Option<Address>` only for a genuinely
    // optional account (`acc.optional`). Every declared account gets exactly
    // one meta in declared order, matching `#[derive(Accounts)]`'s own
    // sentinel scheme (`naclac-macros/src/accounts.rs`): a `None` optional
    // is filled with `program_id` rather than omitted, since omitting it
    // would shift every subsequent account's position — same requirement
    // the CPI generator's builder already honors (`instructions/cpi.rs`).
    // Every other field, including ones with a well-known resolved address
    // (e.g. `system_program`/`rent`), is a plain mandatory `Address` the
    // caller always supplies explicitly.
    if !ix.accounts.is_empty() {
        offchain_content.push_str("#[cfg(feature = \"offchain\")]\n");
        offchain_content.push_str(&super::super::render_docs(&ix.docs, ""));
        offchain_content.push_str(&format!("pub struct {}Accounts {{\n", ix_camel));
        for acc in &ix.accounts {
            let acc_snake = AsSnakeCase(&acc.name).to_string();
            let is_optional = acc.optional.unwrap_or(false);
            offchain_content.push_str(&super::super::render_docs(&acc.docs, "    "));
            if is_optional {
                offchain_content.push_str(&format!(
                    "    pub {}: Option<naclac_client::Address>,\n",
                    acc_snake
                ));
            } else {
                offchain_content
                    .push_str(&format!("    pub {}: naclac_client::Address,\n", acc_snake));
            }
        }
        offchain_content.push_str("}\n\n");
    }

    // Builder function signature
    let mut builder_args = vec![
        "provider: &'a naclac_client::NaclacProvider".to_string(),
        "program_id: naclac_client::Address".to_string(),
    ];

    for arg in &ix.args {
        let arg_snake = AsSnakeCase(&arg.name).to_string();
        let arg_ty =
            super::super::map_type_to_rust_with_prefix(&arg.ty, idl.is_zero_copy, "crate::types::");
        builder_args.push(format!("{}: {}", arg_snake, arg_ty));
    }

    if !ix.accounts.is_empty() {
        builder_args.push(format!("accounts: {}Accounts", ix_camel));
    }

    offchain_content.push_str("#[cfg(feature = \"offchain\")]\n");
    offchain_content.push_str(&super::super::render_docs(&ix.docs, ""));
    offchain_content.push_str(&format!(
        "pub fn build_{}<'a>(\n    {},\n) -> naclac_client::InstructionBuilder<'a> {{\n",
        ix_snake,
        builder_args.join(",\n    ")
    ));

    // Serialization logic
    let disc_str = ix
        .discriminator
        .iter()
        .map(|b| b.to_string())
        .collect::<Vec<_>>()
        .join(", ");
    if ix.args.is_empty() {
        offchain_content.push_str(&format!(
            "    let ix_data = crate::sdk_core_offchain::vec![{}];\n",
            disc_str
        ));
    } else {
        offchain_content.push_str(&format!(
            "    let mut ix_data = crate::sdk_core_offchain::vec![{}];\n",
            disc_str
        ));
    }

    if !ix.args.is_empty() {
        offchain_content.push_str(&format!("    let args = {}IxArgs {{\n", ix_camel));
        for arg in &ix.args {
            let arg_snake = AsSnakeCase(&arg.name).to_string();
            offchain_content.push_str(&format!("        {},\n", arg_snake));
        }
        offchain_content.push_str("    };\n");

        offchain_content.push_str("    #[cfg(not(feature = \"borsh\"))]\n    {\n");
        offchain_content.push_str(&super::super::generate_zero_copy_arg_bytes(
            ix,
            false,
            idl.is_zero_copy,
            &idl.defined_types,
        ));
        offchain_content.push_str("    }\n");
        offchain_content.push_str(
            "    #[cfg(feature = \"borsh\")]\n\
             \x20   {\n\
             \x20       crate::sdk_core_offchain::borsh::BorshSerialize::serialize(&args, &mut ix_data).unwrap();\n\
             \x20   }\n"
        );
    }

    // Build the instruction builder. Single pass, in declared order — an
    // optional account's address resolves to `program_id` (the sentinel)
    // when `None`, rather than being skipped, so every account keeps its
    // declared position regardless of which optionals are present.
    offchain_content
        .push_str("    naclac_client::InstructionBuilder::new(provider, program_id, ix_data)\n");
    for acc in &ix.accounts {
        let acc_snake = AsSnakeCase(&acc.name).to_string();
        let address_expr = if acc.optional.unwrap_or(false) {
            format!("accounts.{acc_snake}.unwrap_or(program_id)")
        } else {
            format!("accounts.{acc_snake}")
        };
        offchain_content.push_str(&format!(
            "        .account(naclac_client::AccountMeta {{ address: {address_expr}, is_signer: {}, is_writable: {} }}, \"{}\")\n",
            acc.signer, acc.writable, acc.name
        ));
    }
    offchain_content.push_str("}\n\n");

    offchain_content
}
