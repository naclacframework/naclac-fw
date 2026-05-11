use std::fs;
use std::path::Path;
use std::process::Command;

pub fn ensure_dir(path: &Path) {
    if !path.exists() {
        fs::create_dir_all(path).expect("Failed to create directory");
    }
}

pub fn write_file(path: &Path, content: &str) {
    fs::write(path, content).expect(&format!("Failed to write file: {:?}", path));
}

pub fn generate_keypair(keypair_path: &Path) -> String {
    println!("🔑 Generating cryptographic root identity...");
    Command::new("solana-keygen")
        .arg("new")
        .arg("--no-bip39-passphrase")
        .arg("-o")
        .arg(keypair_path)
        .arg("--force")
        .output()
        .expect("Failed to initialize system program keypair.");

    let pubkey_output = Command::new("solana-keygen")
        .arg("pubkey")
        .arg(keypair_path)
        .output()
        .expect("Failed to derive valid public address from generator hash.");

    String::from_utf8_lossy(&pubkey_output.stdout).trim().to_string()
}
