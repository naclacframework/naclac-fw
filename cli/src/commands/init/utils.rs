use {
    crate::ui,
    solana_keypair::Keypair,
    solana_signer::Signer,
    std::{
        fs::{self, OpenOptions},
        io::Write,
        path::Path,
        process::Command,
    },
};

pub fn ensure_dir(path: &Path) {
    if !path.exists() {
        fs::create_dir_all(path).unwrap_or_else(|e| {
            ui::error(format!(
                "Failed to create directory {}: {e}",
                path.display()
            ))
        });
    }
}

pub fn write_file(path: &Path, content: &str) {
    fs::write(path, content)
        .unwrap_or_else(|e| ui::error(format!("Failed to write {}: {e}", path.display())));
}

/// Writes `keypair` to `keypair_path` as the JSON array of 64 bytes that
/// `solana-keygen` writes, readable only by its owner on Unix, and returns its
/// address.
pub fn write_keypair(keypair: &Keypair, keypair_path: &Path) -> String {
    if let Some(parent) = keypair_path.parent() {
        ensure_dir(parent);
    }
    let text = serde_json::to_string(&keypair.to_bytes().to_vec())
        .unwrap_or_else(|e| ui::error(format!("Cannot encode the keypair: {e}")));
    let mut options = OpenOptions::new();
    options.write(true).create(true).truncate(true);
    #[cfg(unix)]
    std::os::unix::fs::OpenOptionsExt::mode(&mut options, 0o600);
    options
        .open(keypair_path)
        .and_then(|mut file| file.write_all(text.as_bytes()))
        .unwrap_or_else(|e| ui::error(format!("Cannot write {}: {e}", keypair_path.display())));
    keypair.pubkey().to_string()
}

pub fn generate_keypair(keypair_path: &Path) -> String {
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
