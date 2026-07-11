use crate::Idl;
use heck::{AsSnakeCase, ToUpperCamelCase};
use std::fs;
use std::path::Path;

pub mod cpi;
pub mod offchain;

pub fn generate_instructions(
    idl: &Idl,
    clients_dir: &Path,
    header: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    if idl.instructions.is_empty() {
        return Ok(());
    }

    // Generate the instructions/mod.rs file in the client
    let mut instructions_mod = header.to_string();
    for ix in &idl.instructions {
        let ix_snake = AsSnakeCase(&ix.name).to_string();
        instructions_mod.push_str(&format!("pub mod {};\n", ix_snake));
        instructions_mod.push_str(&format!("pub use {}::*;\n", ix_snake));
    }
    fs::write(
        clients_dir.join("src/instructions/mod.rs"),
        instructions_mod,
    )
    .unwrap();

    // Generate each instruction file
    for ix in &idl.instructions {
        let ix_snake = AsSnakeCase(&ix.name).to_string();
        let ix_camel = ix.name.to_upper_camel_case();

        let mut file_content = header.to_string();

        // 1. Generate clean IxArgs struct at the top (compiled in both modes if args are present)
        if !ix.args.is_empty() {
            file_content.push_str("#[cfg(feature = \"borsh\")]\n");
            file_content.push_str("use crate::sdk_core::borsh::BorshSerialize;\n\n");

            file_content.push_str(&format!(
                "#[cfg_attr(feature = \"borsh\", derive(Clone, Debug, BorshSerialize))]\n\
                 #[cfg_attr(feature = \"borsh\", borsh(crate = \"crate::sdk_core::borsh\"))]\n\
                 #[cfg_attr(not(feature = \"borsh\"), derive(Copy, Clone, Debug))]\n\
                 #[cfg_attr(not(feature = \"borsh\"), repr(C, packed))]\n\
                 pub struct {}IxArgs {{\n",
                ix_camel
            ));

            for arg in &ix.args {
                let arg_snake = AsSnakeCase(&arg.name).to_string();
                let arg_ty = super::map_type_to_rust_with_prefix(
                    &arg.ty,
                    idl.is_zero_copy,
                    "crate::types::",
                );
                file_content.push_str(&format!("    pub {}: {},\n", arg_snake, arg_ty));
            }
            file_content.push_str("}\n\n");

            file_content.push_str("#[cfg(not(feature = \"borsh\"))]\n");
            file_content.push_str(&format!(
                "unsafe impl crate::sdk_core::bytemuck::Zeroable for {}IxArgs {{}}\n",
                ix_camel
            ));
            file_content.push_str("#[cfg(not(feature = \"borsh\"))]\n");
            file_content.push_str(&format!(
                "unsafe impl crate::sdk_core::bytemuck::Pod for {}IxArgs {{}}\n\n",
                ix_camel
            ));
        }

        // 2. Call off-chain instruction helper
        let offchain_fragment = offchain::generate_offchain_ix(ix, idl);
        file_content.push_str(&offchain_fragment);

        // 3. Call CPI instruction helper
        let cpi_fragment = cpi::generate_cpi_ix(ix, idl);
        file_content.push_str(&cpi_fragment);

        fs::write(
            clients_dir.join(format!("src/instructions/{}.rs", ix_snake)),
            file_content,
        )
        .unwrap();
    }

    Ok(())
}
