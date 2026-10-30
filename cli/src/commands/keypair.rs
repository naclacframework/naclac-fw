//! Reads Solana keypair files (the 64-byte JSON array `solana-keygen`
//! writes) for every command that needs a wallet or deploy keypair.

use {
    solana_keypair::Keypair,
    std::{fs, path::Path},
};

const KEYPAIR_FILE_LEN: usize = 64;

/// Reads the keypair file at `path`. Errors unless the file is a JSON array of
/// exactly 64 bytes whose last 32 bytes are the public key of the first 32.
pub fn read(path: &Path) -> Result<Keypair, String> {
    let text = fs::read_to_string(path)
        .map_err(|e| format!("Cannot read the keypair file {}: {e}", path.display()))?;
    let bytes: Vec<u8> = serde_json::from_str(&text).map_err(|e| {
        format!(
            "{} is not a keypair file (a JSON array of {KEYPAIR_FILE_LEN} bytes): {e}",
            path.display()
        )
    })?;
    if bytes.len() != KEYPAIR_FILE_LEN {
        return Err(format!(
            "{} holds {} bytes; a keypair file has exactly {KEYPAIR_FILE_LEN}.",
            path.display(),
            bytes.len()
        ));
    }
    Keypair::try_from(bytes.as_slice())
        .map_err(|e| format!("{} is not a valid keypair: {e}", path.display()))
}
