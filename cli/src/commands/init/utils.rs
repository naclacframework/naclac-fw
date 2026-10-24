use {
    crate::ui,
    std::{fs, path::Path, process::Command},
};

pub fn ensure_dir(path: &Path) {
    if !path.exists() {
        fs::create_dir_all(path).expect("Failed to create directory");
    }
}

pub fn write_file(path: &Path, content: &str) {
    fs::write(path, content).unwrap_or_else(|_| panic!("Failed to write file: {:?}", path));
}

pub fn generate_keypair(keypair_path: &Path) -> String {
    println!("🔑 Generating cryptographic root identity...");
    if let Some(parent) = keypair_path.parent() {
        ensure_dir(parent);
    }
    let keygen = Command::new("solana-keygen")
        .arg("new")
        .arg("--no-bip39-passphrase")
        .arg("-o")
        .arg(keypair_path)
        .arg("--force")
        .output()
        .unwrap_or_else(|e| ui::error(format!("Failed to run solana-keygen: {}", e)));
    if !keygen.status.success() {
        ui::error(format!(
            "solana-keygen failed to generate {}: {}",
            keypair_path.display(),
            String::from_utf8_lossy(&keygen.stderr).trim()
        ));
    }

    let address_output = Command::new("solana-keygen")
        .arg("pubkey")
        .arg(keypair_path)
        .output()
        .unwrap_or_else(|e| ui::error(format!("Failed to run solana-keygen: {}", e)));
    let address = String::from_utf8_lossy(&address_output.stdout)
        .trim()
        .to_string();
    if !address_output.status.success() || address.is_empty() {
        ui::error(format!(
            "Could not read the program address from {}: {}",
            keypair_path.display(),
            String::from_utf8_lossy(&address_output.stderr).trim()
        ));
    }
    address
}
