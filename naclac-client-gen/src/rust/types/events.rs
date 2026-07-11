use super::super::map_type_to_rust;
use crate::Idl;
use heck::{AsSnakeCase, ToUpperCamelCase};
use std::fs;
use std::path::Path;

pub fn generate(
    idl: &Idl,
    clients_dir: &Path,
    header: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    if idl.events.is_empty() {
        return Ok(());
    }

    let mut events_content = header.to_string();
    events_content.push_str("#[cfg(feature = \"borsh\")]\n");
    events_content.push_str("use crate::sdk_core::borsh::{BorshDeserialize, BorshSerialize};\n\n");

    for event in &idl.events {
        let event_camel = event.name.to_upper_camel_case();

        events_content.push_str(&format!(
            "#[cfg_attr(feature = \"borsh\", derive(Clone, Debug, BorshSerialize, BorshDeserialize))]\n\
             #[cfg_attr(feature = \"borsh\", borsh(crate = \"crate::sdk_core::borsh\"))]\n\
             #[cfg_attr(not(feature = \"borsh\"), derive(Copy, Clone, Debug))]\n\
             #[cfg_attr(not(feature = \"borsh\"), repr(C))]\n\
             pub struct {} {{\n",
            event_camel
        ));

        for field in &event.fields {
            let f_snake = AsSnakeCase(&field.name).to_string();
            let f_ty = map_type_to_rust(&field.ty, idl.is_zero_copy);
            events_content.push_str(&format!("    pub {}: {},\n", f_snake, f_ty));
        }
        events_content.push_str("}\n\n");

        events_content.push_str("#[cfg(not(feature = \"borsh\"))]\n");
        events_content.push_str(&format!(
            "unsafe impl crate::sdk_core::bytemuck::Zeroable for {} {{}}\n",
            event_camel
        ));
        events_content.push_str("#[cfg(not(feature = \"borsh\"))]\n");
        events_content.push_str(&format!(
            "unsafe impl crate::sdk_core::bytemuck::Pod for {} {{}}\n\n",
            event_camel
        ));
    }

    fs::write(clients_dir.join("src/types/events.rs"), events_content).unwrap();
    Ok(())
}
