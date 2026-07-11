use crate::Idl;
use std::fs;

pub mod kit;
pub mod legacy;
pub mod shared;

pub use shared::generate_ts;

pub fn generate_typescript_sdk(
    idl_json: &str,
    program_name: &str,
    workspace_root: &std::path::Path,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut idl: Idl = serde_json::from_str(idl_json)?;
    let temp_marker = workspace_root.join(format!("target/.{}-zero-copy", program_name));
    if temp_marker.exists() {
        idl.is_zero_copy = true;
    }

    let clients_dir =
        workspace_root.join(format!("clients/typescript/src/generated/{}", program_name));
    if clients_dir.exists() {
        fs::remove_dir_all(&clients_dir).unwrap();
    }
    fs::create_dir_all(clients_dir.join("instructions")).unwrap();
    fs::create_dir_all(clients_dir.join("accounts/kit")).unwrap();
    fs::create_dir_all(clients_dir.join("accounts/legacy")).unwrap();
    fs::create_dir_all(clients_dir.join("types")).unwrap();
    fs::create_dir_all(clients_dir.join("idl")).unwrap();

    let target_types_dir = workspace_root.join("target/types");
    let ts_idl_path = target_types_dir.join(format!("{}.ts", program_name));
    if ts_idl_path.exists() {
        fs::copy(
            &ts_idl_path,
            clients_dir.join(format!("idl/{}.ts", program_name)),
        )
        .unwrap();
    }
    let json_idl_path = workspace_root.join(format!("target/idl/{}.json", program_name));
    if json_idl_path.exists() {
        fs::copy(
            &json_idl_path,
            clients_dir.join(format!("idl/{}.json", program_name)),
        )
        .unwrap();
    }

    // 1. Generate Shared Components (types, instructions, accounts, events, index barrel)
    shared::generate_shared_files(&idl, program_name, &clients_dir)?;

    // 2. Generate Modern Kit Client (client.ts)
    kit::generate_kit_client(&idl, program_name, &clients_dir)?;

    // 3. Generate Legacy Web3.js Client (client_legacy.ts)
    legacy::generate_legacy_client(&idl, program_name, &clients_dir)?;

    println!(
        "✅ Client SDK successfully generated in clients/typescript/src/generated/{}",
        program_name
    );
    Ok(())
}
