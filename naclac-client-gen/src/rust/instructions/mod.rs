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

        // 1. Generate the offchain and cpi IxArgs structs — always both,
        // independently named (`{Ix}IxArgs` / `{Ix}CpiIxArgs`) and
        // independently cfg-gated. `sdk_core_offchain`/`sdk_core_cpi` can
        // resolve to genuinely different underlying types (Address, Bool,
        // ZcString vs String, ...) for the exact same IDL type, so these
        // structs can never safely share one name — a downstream crate that
        // needs both `offchain` and `cpi` active at once (e.g. its own tests
        // CPI-calling a different program) would otherwise get whichever
        // variant's cfg happened to win, silently breaking the other.
        if !ix.args.is_empty() {
            let pod_only = super::is_pod_only_args(ix, &idl.defined_types);

            for (for_cpi, cfg_gate, struct_suffix) in [
                (false, "#[cfg(feature = \"offchain\")]\n", "IxArgs"),
                (true, "#[cfg(feature = \"cpi\")]\n", "CpiIxArgs"),
            ] {
                let sdk_core = super::sdk_core_alias(for_cpi);
                let struct_name = format!("{}{}", ix_camel, struct_suffix);

                file_content.push_str(cfg_gate);
                file_content.push_str(&format!(
                    "#[cfg(feature = \"borsh\")]\nuse {}::borsh::BorshSerialize;\n",
                    sdk_core
                ));

                file_content.push_str(cfg_gate);
                if pod_only {
                    file_content.push_str(&format!(
                        "#[cfg_attr(feature = \"borsh\", derive(Clone, Debug, BorshSerialize))]\n\
                         #[cfg_attr(feature = \"borsh\", borsh(crate = \"{sdk_core}::borsh\"))]\n\
                         #[cfg_attr(not(feature = \"borsh\"), derive(Copy, Clone, Debug))]\n\
                         #[cfg_attr(not(feature = \"borsh\"), repr(C, packed))]\n\
                         pub struct {struct_name} {{\n"
                    ));
                } else {
                    file_content.push_str(&format!(
                        "#[cfg_attr(feature = \"borsh\", derive(Clone, Debug, BorshSerialize))]\n\
                         #[cfg_attr(feature = \"borsh\", borsh(crate = \"{sdk_core}::borsh\"))]\n\
                         #[cfg_attr(not(feature = \"borsh\"), derive(Clone, Debug))]\n\
                         pub struct {struct_name} {{\n"
                    ));
                }

                for arg in &ix.args {
                    let arg_snake = AsSnakeCase(&arg.name).to_string();
                    let arg_ty = if for_cpi {
                        super::map_type_to_rust_cpi(&arg.ty, idl.is_zero_copy, "crate::types::")
                    } else {
                        super::map_type_to_rust_with_prefix(
                            &arg.ty,
                            idl.is_zero_copy,
                            "crate::types::",
                        )
                    };
                    file_content.push_str(&format!("    pub {}: {},\n", arg_snake, arg_ty));
                }
                file_content.push_str("}\n\n");

                if pod_only {
                    file_content.push_str(cfg_gate);
                    file_content.push_str("#[cfg(not(feature = \"borsh\"))]\n");
                    file_content.push_str(&format!(
                        "unsafe impl {sdk_core}::bytemuck::Zeroable for {struct_name} {{}}\n"
                    ));
                    file_content.push_str(cfg_gate);
                    file_content.push_str("#[cfg(not(feature = \"borsh\"))]\n");
                    file_content.push_str(&format!(
                        "unsafe impl {sdk_core}::bytemuck::Pod for {struct_name} {{}}\n\n"
                    ));
                }
            }
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
