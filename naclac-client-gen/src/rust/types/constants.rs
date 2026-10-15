use super::super::{map_type_to_rust, render_docs};
use crate::Idl;
use std::fs;
use std::path::Path;

pub fn generate(
    idl: &Idl,
    clients_dir: &Path,
    header: &str,
    address_bytes_str: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut constants_content = header.to_string();
    // Independently gated and independently named (never a mutually
    // exclusive `offchain`/`not(offchain)` pair sharing one name): `offchain`
    // and `cpi` aren't mutually exclusive at the Cargo level (feature
    // unification can activate both in one build, e.g. a normal `cpi`-only
    // dependency and an `offchain`-only dev-dependency on the same package),
    // and `naclac_client::Address`/`crate::sdk_core_cpi::Address` are
    // genuinely different, non-interchangeable types -- a shared name would
    // silently resolve to whichever cfg'd variant happened to compile,
    // breaking the other. Mirrors `types/typedefs.rs`/`components.rs`'s
    // `X`/`XCpi` split.
    constants_content.push_str(&format!(
        "#[cfg(feature = \"offchain\")]\n\
         pub const PROGRAM_ID: naclac_client::Address = naclac_client::Address::new_from_array([\n    {}\n]);\n\n\
         #[cfg(feature = \"cpi\")]\n\
         pub const PROGRAM_ID_CPI: crate::sdk_core_cpi::Address = crate::sdk_core_cpi::Address::new_from_array([\n    {}\n]);\n\n",
        address_bytes_str, address_bytes_str
    ));
    for constant in &idl.constants {
        // Clean constant values
        let raw_val = constant.value.trim();
        let is_decimal_byte_array = raw_val.starts_with('[') && raw_val.ends_with(']');
        let array_len = constant
            .ty
            .get("array")
            .and_then(|a| a.get(1))
            .and_then(|n| n.as_u64());

        // `publicKey`-typed constants need the same offchain/cpi split as
        // `PROGRAM_ID` above: `Address` resolves to a different, non-
        // interchangeable type per alias, but every other constant type
        // (u8/u64/[u8; N]/...) renders identically regardless, so only this
        // branch needs cfg-gating -- and, like `PROGRAM_ID`, independently
        // named (`{name}` / `{name}_CPI`), never a mutually exclusive
        // `offchain`/`not(offchain)` pair sharing one name.
        if constant.ty == "publicKey" {
            let decoded = bs58::decode(raw_val).into_vec().unwrap_or_else(|_| {
                panic!(
                    "Naclac Error: constant '{}' is publicKey-typed but its value '{}' is not valid base58",
                    constant.name, raw_val
                )
            });
            let bytes_str = decoded
                .iter()
                .map(|b| b.to_string())
                .collect::<Vec<_>>()
                .join(", ");

            constants_content.push_str(&render_docs(&constant.docs, ""));
            constants_content.push_str(&format!(
                "#[cfg(feature = \"offchain\")]\n\
                 pub const {name}: naclac_client::Address = naclac_client::Address::new_from_array([{bytes}]);\n\
                 #[cfg(feature = \"cpi\")]\n\
                 pub const {name}_CPI: crate::sdk_core_cpi::Address = crate::sdk_core_cpi::Address::new_from_array([{bytes}]);\n",
                name = constant.name,
                bytes = bytes_str,
            ));
            continue;
        }

        let (ty, clean_val) = if is_decimal_byte_array && constant.ty == "bytes" {
            // `bytes`-typed constant with a decimal byte-array value -> &[u8] = &[...]
            ("&[u8]".to_string(), format!("&{}", raw_val))
        } else if is_decimal_byte_array && array_len.is_some() {
            // `{"array": ["u8", N]}`-typed constant -> [u8; N] = [...]
            (format!("[u8; {}]", array_len.unwrap()), raw_val.to_string())
        } else {
            let ty = map_type_to_rust(&constant.ty, idl.is_zero_copy);
            let val = raw_val
                .trim_end_matches("u64")
                .trim_end_matches("u128")
                .trim_end_matches("i64")
                .trim_end_matches("i128")
                .trim_end_matches("usize")
                .trim_end_matches("isize")
                .to_string();
            (ty, val)
        };

        constants_content.push_str(&render_docs(&constant.docs, ""));
        constants_content.push_str(&format!(
            "pub const {}: {} = {};\n",
            constant.name, ty, clean_val
        ));
    }
    fs::write(
        clients_dir.join("src/types/constants.rs"),
        constants_content,
    )
    .unwrap();
    Ok(())
}
