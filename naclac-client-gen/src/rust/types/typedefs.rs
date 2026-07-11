use super::super::map_type_to_rust;
use crate::{Idl, IdlTypeDefVariants};
use heck::{AsSnakeCase, ToUpperCamelCase};
use std::fs;
use std::path::Path;

pub fn generate(
    idl: &Idl,
    clients_dir: &Path,
    header: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    if idl.defined_types.is_empty() {
        return Ok(());
    }

    let mut typedefs_content = header.to_string();
    typedefs_content.push_str("#[cfg(feature = \"borsh\")]\n");
    typedefs_content
        .push_str("use crate::sdk_core::borsh::{BorshDeserialize, BorshSerialize};\n\n");

    for t in &idl.defined_types {
        let name_camel = t.name.to_upper_camel_case();
        match &t.ty {
            IdlTypeDefVariants::Enum { variants } => {
                typedefs_content.push_str(&format!(
                    "#[cfg_attr(feature = \"borsh\", derive(BorshSerialize, BorshDeserialize))]\n\
                     #[cfg_attr(feature = \"borsh\", borsh(crate = \"crate::sdk_core::borsh\"))]\n\
                     #[derive(Clone, Copy, Debug, PartialEq, Eq)]\n\
                     #[repr(u8)]\n\
                     pub enum {} {{\n",
                    name_camel
                ));
                for v in variants {
                    typedefs_content.push_str(&format!("    {},\n", v.name.to_upper_camel_case()));
                }
                typedefs_content.push_str("}\n\n");
            }
            IdlTypeDefVariants::Struct { fields } => {
                typedefs_content.push_str(&format!(
                    "#[cfg_attr(feature = \"borsh\", derive(Clone, Debug, BorshSerialize, BorshDeserialize))]\n\
                     #[cfg_attr(feature = \"borsh\", borsh(crate = \"crate::sdk_core::borsh\"))]\n\
                     #[cfg_attr(not(feature = \"borsh\"), derive(Copy, Clone, Debug))]\n\
                     #[cfg_attr(not(feature = \"borsh\"), repr(C))]\n\
                     pub struct {} {{\n",
                    name_camel
                ));

                for f in fields {
                    let f_snake = AsSnakeCase(&f.name).to_string();
                    let f_ty = map_type_to_rust(&f.ty, idl.is_zero_copy);
                    typedefs_content.push_str(&format!("    pub {}: {},\n", f_snake, f_ty));
                }
                typedefs_content.push_str("}\n\n");

                typedefs_content.push_str("#[cfg(not(feature = \"borsh\"))]\n");
                typedefs_content.push_str(&format!(
                    "unsafe impl crate::sdk_core::bytemuck::Zeroable for {} {{}}\n",
                    name_camel
                ));
                typedefs_content.push_str("#[cfg(not(feature = \"borsh\"))]\n");
                typedefs_content.push_str(&format!(
                    "unsafe impl crate::sdk_core::bytemuck::Pod for {} {{}}\n\n",
                    name_camel
                ));
            }
        }
    }
    fs::write(clients_dir.join("src/types/typedefs.rs"), typedefs_content).unwrap();

    Ok(())
}
