use super::super::map_type_to_rust;
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
    constants_content.push_str(&format!(
        "#[cfg(feature = \"offchain\")]\n\
         pub const PROGRAM_ID: naclac_client::Address = naclac_client::Address::new_from_array([\n    {}\n]);\n\n\
         #[cfg(not(feature = \"offchain\"))]\n\
         pub const PROGRAM_ID: crate::sdk_core::Address = crate::sdk_core::Address::new_from_array([\n    {}\n]);\n\n",
        address_bytes_str, address_bytes_str
    ));
    for constant in &idl.constants {
        // Clean constant values
        let raw_val = constant.value.trim();
        let (ty, clean_val) = if raw_val.starts_with("b\"") && raw_val.ends_with("\"") {
            // Byte string literal b"..." -> &[u8] = &[...]
            let inner = &raw_val[2..raw_val.len() - 1];
            let bytes_str = inner
                .as_bytes()
                .iter()
                .map(|b| b.to_string())
                .collect::<Vec<_>>()
                .join(", ");
            ("&[u8]".to_string(), format!("&[{}]", bytes_str))
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
