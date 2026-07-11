use super::map_type_to_rust;
use crate::Idl;
use heck::{AsSnakeCase, ToUpperCamelCase};
use std::fs;
use std::path::Path;

pub fn generate_components(
    idl: &Idl,
    clients_dir: &Path,
    header: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    if idl.accounts.is_empty() {
        return Ok(());
    }

    let mut components_mod = header.to_string();
    for acc in &idl.accounts {
        let acc_snake = AsSnakeCase(&acc.name).to_string();
        components_mod.push_str(&format!("pub mod {};\n", acc_snake));
        components_mod.push_str(&format!("pub use {}::*;\n", acc_snake));
    }
    fs::write(clients_dir.join("src/components/mod.rs"), components_mod).unwrap();

    for acc in &idl.accounts {
        let acc_snake = AsSnakeCase(&acc.name).to_string();
        let acc_camel = acc.name.to_upper_camel_case();

        let mut comp_content = header.to_string();
        comp_content.push_str("#[cfg(feature = \"borsh\")]\n");
        comp_content
            .push_str("use crate::sdk_core::borsh::{BorshDeserialize, BorshSerialize};\n\n");

        comp_content.push_str(&format!(
            "#[cfg_attr(feature = \"borsh\", derive(Clone, Debug, BorshSerialize, BorshDeserialize))]\n\
             #[cfg_attr(feature = \"borsh\", borsh(crate = \"crate::sdk_core::borsh\"))]\n\
             #[cfg_attr(not(feature = \"borsh\"), derive(Copy, Clone, Debug))]\n\
             #[cfg_attr(not(feature = \"borsh\"), repr(C))]\n\
             pub struct {} {{\n",
            acc_camel
        ));

        for field in &acc.ty.fields {
            let field_snake = AsSnakeCase(&field.name).to_string();
            let field_ty = map_type_to_rust(&field.ty, idl.is_zero_copy);
            comp_content.push_str(&format!("    pub {}: {},\n", field_snake, field_ty));
        }
        comp_content.push_str("}\n");

        // Discriminator check helper
        if let Some(disc) = &acc.discriminator {
            let disc_bytes = disc
                .iter()
                .map(|b| b.to_string())
                .collect::<Vec<_>>()
                .join(", ");
            let disc_name = acc.name.to_uppercase().replace(' ', "_");
            comp_content.push_str(&format!(
                "\n/// 8-byte on-chain discriminator for `{}` accounts.\n\
                 pub const {}_DISCRIMINATOR: [u8; 8] = [{}];\n",
                acc_camel, disc_name, disc_bytes,
            ));

            // A single `AccountFetcher::fetch::<{acc_camel}>()` call site works
            // correctly regardless of Borsh vs zero-copy mode: the decode
            // strategy is picked here, at THIS type's own compile time (via the
            // generated client crate's own `borsh` feature, set correctly from
            // the real IDL at generation time), not guessed at runtime.
            comp_content.push_str(&format!(
                r#"#[cfg(feature = "offchain")]
#[cfg(not(feature = "borsh"))]
impl naclac_client::NaclacDecode for {acc_camel} {{
    const DISCRIMINATOR: Option<[u8; 8]> = Some({disc_name}_DISCRIMINATOR);
    fn naclac_decode(data: &[u8]) -> Result<Self, naclac_client::NaclacClientError> {{
        naclac_client::decode_pod_checked::<Self>(data, {disc_name}_DISCRIMINATOR)
    }}
}}

#[cfg(feature = "offchain")]
#[cfg(feature = "borsh")]
impl naclac_client::NaclacDecode for {acc_camel} {{
    const DISCRIMINATOR: Option<[u8; 8]> = Some({disc_name}_DISCRIMINATOR);
    fn naclac_decode(data: &[u8]) -> Result<Self, naclac_client::NaclacClientError> {{
        naclac_client::decode_borsh_checked::<Self>(data, {disc_name}_DISCRIMINATOR)
    }}
}}
"#,
                acc_camel = acc_camel,
                disc_name = disc_name,
            ));
        }

        // Bytemuck impls (required for Zero-Copy / no-Borsh mode)
        comp_content.push_str("#[cfg(not(feature = \"borsh\"))]\n");
        comp_content.push_str(&format!(
            "unsafe impl crate::sdk_core::bytemuck::Zeroable for {} {{}}\n",
            acc_camel
        ));
        comp_content.push_str("#[cfg(not(feature = \"borsh\"))]\n");
        comp_content.push_str(&format!(
            "unsafe impl crate::sdk_core::bytemuck::Pod for {} {{}}\n",
            acc_camel
        ));

        // Network-fetching convenience functions. Identical in both modes —
        // each delegates entirely to `{acc_camel}`'s own `NaclacDecode` impl
        // above (the single source of truth for how to decode this type), so
        // there's no independent borsh/bytemuck logic here to drift out of
        // sync with it. Kept as named, discoverable per-type functions
        // (rather than only the generic `AccountFetcher`) because that's the
        // actual value of a generated SDK: auto-specializing the generic
        // framework tooling for this one program's own types. Anyone with
        // account bytes from somewhere other than `provider` (a simulation
        // result, a cached blob, ...) can decode them directly via
        // `<{acc_camel} as naclac_client::NaclacDecode>::naclac_decode(&bytes)`.
        comp_content.push_str(&format!(
            r#"#[cfg(feature = "offchain")]
/// Fetches and decodes a `{acc_camel}` account from the chain.
pub fn fetch_{acc_snake}(
    provider: &naclac_client::NaclacProvider,
    address: &naclac_client::Address,
) -> Result<{acc_camel}, naclac_client::NaclacClientError> {{
    let raw = provider.get_account_data(address)?;
    <{acc_camel} as naclac_client::NaclacDecode>::naclac_decode(&raw)
}}

#[cfg(feature = "offchain")]
/// Fetches a `{acc_camel}` account. Returns `None` if the account does not exist.
pub fn fetch_maybe_{acc_snake}(
    provider: &naclac_client::NaclacProvider,
    address: &naclac_client::Address,
) -> Result<Option<{acc_camel}>, naclac_client::NaclacClientError> {{
    match fetch_{acc_snake}(provider, address) {{
        Ok(a) => Ok(Some(a)),
        Err(naclac_client::NaclacClientError::AccountNotFound(_)) => Ok(None),
        Err(e) => Err(e),
    }}
}}

#[cfg(feature = "offchain")]
/// Fetches and decodes every on-chain `{acc_camel}` account owned by `program_id`.
pub fn fetch_all_{acc_snake}(
    provider: &naclac_client::NaclacProvider,
    program_id: &naclac_client::Address,
) -> Result<crate::sdk_core::Vec<(naclac_client::Address, {acc_camel})>, naclac_client::NaclacClientError> {{
    let raw_accounts = provider.get_program_accounts(program_id, Some(&{disc_name}_DISCRIMINATOR))?;
    let mut results = crate::sdk_core::Vec::new();
    for (address, data) in raw_accounts {{
        if let Ok(value) = <{acc_camel} as naclac_client::NaclacDecode>::naclac_decode(&data) {{
            results.push((address, value));
        }}
    }}
    Ok(results)
}}
"#,
            acc_camel = acc_camel,
            acc_snake = acc_snake,
            disc_name = acc.name.to_uppercase().replace(' ', "_"),
        ));

        fs::write(
            clients_dir.join(format!("src/components/{}.rs", acc_snake)),
            comp_content,
        )
        .unwrap();
    }

    Ok(())
}
