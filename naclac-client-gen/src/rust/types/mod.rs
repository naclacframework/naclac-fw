use crate::Idl;
use std::fs;
use std::path::Path;

pub mod constants;
pub mod errors;
pub mod events;
pub mod typedefs;

pub fn generate_types(
    idl: &Idl,
    clients_dir: &Path,
    header: &str,
    address_bytes_str: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut types_mod = header.to_string();
    if !idl.defined_types.is_empty() {
        types_mod.push_str("pub mod typedefs;\npub use typedefs::*;\n");
    }
    types_mod.push_str("pub mod constants;\npub use constants::*;\n");
    if !idl.events.is_empty() {
        types_mod.push_str("pub mod events;\npub use events::*;\n");
    }
    if !idl.errors.is_empty() {
        types_mod.push_str("pub mod errors;\npub use errors::*;\n");
    }
    fs::write(clients_dir.join("src/types/mod.rs"), types_mod).unwrap();

    // Call sub-generators
    constants::generate(idl, clients_dir, header, address_bytes_str)?;
    errors::generate(idl, clients_dir, header)?;
    events::generate(idl, clients_dir, header)?;
    typedefs::generate(idl, clients_dir, header)?;

    Ok(())
}
