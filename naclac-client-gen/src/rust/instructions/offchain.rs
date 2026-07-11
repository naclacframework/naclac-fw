use crate::{Idl, IdlInstruction};
use heck::{AsSnakeCase, ToUpperCamelCase};

pub fn generate_offchain_ix(ix: &IdlInstruction, idl: &Idl) -> String {
    let mut offchain_content = String::new();
    let ix_snake = AsSnakeCase(&ix.name).to_string();
    let ix_camel = ix.name.to_upper_camel_case();

    // Accounts struct for off-chain
    if !ix.accounts.is_empty() {
        offchain_content.push_str("#[cfg(feature = \"offchain\")]\n");
        offchain_content.push_str(&format!("pub struct {}Accounts {{\n", ix_camel));
        for acc in &ix.accounts {
            let acc_snake = AsSnakeCase(&acc.name).to_string();
            let is_optional = acc.optional.unwrap_or(false);
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
            "    let ix_data = crate::sdk_core::vec![{}];\n",
            disc_str
        ));
    } else {
        offchain_content.push_str(&format!(
            "    let mut ix_data = crate::sdk_core::vec![{}];\n",
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

        offchain_content.push_str(
            "    #[cfg(not(feature = \"borsh\"))]\n\
             \x20   {\n\
             \x20       ix_data.extend_from_slice(crate::sdk_core::bytemuck::bytes_of(&args));\n\
             \x20   }\n\
             \x20   #[cfg(feature = \"borsh\")]\n\
             \x20   {\n\
             \x20       crate::sdk_core::borsh::BorshSerialize::serialize(&args, &mut ix_data).unwrap();\n\
             \x20   }\n"
        );
    }

    // Build the instruction builder
    let has_optional = ix.accounts.iter().any(|a| a.optional.unwrap_or(false));
    let has_accounts = !ix.accounts.is_empty();

    if has_accounts && has_optional {
        offchain_content.push_str("    let mut builder = naclac_client::InstructionBuilder::new(provider, program_id, ix_data)\n");
        for acc in &ix.accounts {
            let acc_snake = AsSnakeCase(&acc.name).to_string();
            if !acc.optional.unwrap_or(false) {
                offchain_content.push_str(&format!(
                    "        .account(naclac_client::AccountMeta {{ address: accounts.{acc_snake}, is_signer: {}, is_writable: {} }}, \"{}\")\n",
                    acc.signer, acc.writable, acc.name
                ));
            }
        }
        offchain_content.push_str(";\n");
        for acc in &ix.accounts {
            let acc_snake = AsSnakeCase(&acc.name).to_string();
            if acc.optional.unwrap_or(false) {
                offchain_content.push_str(&format!(
                    "    if let Some(addr) = accounts.{acc_snake} {{\n\
                     \x20       builder = builder.account(naclac_client::AccountMeta {{ address: addr, is_signer: {}, is_writable: {} }}, \"{}\");\n\
                     \x20   }}\n",
                    acc.signer, acc.writable, acc.name
                ));
            }
        }
        offchain_content.push_str("    builder\n");
    } else {
        offchain_content.push_str(
            "    naclac_client::InstructionBuilder::new(provider, program_id, ix_data)\n",
        );
        for acc in &ix.accounts {
            let acc_snake = AsSnakeCase(&acc.name).to_string();
            offchain_content.push_str(&format!(
                "        .account(naclac_client::AccountMeta {{ address: accounts.{acc_snake}, is_signer: {}, is_writable: {} }}, \"{}\")\n",
                acc.signer, acc.writable, acc.name
            ));
        }
    }
    offchain_content.push_str("}\n\n");

    offchain_content
}
