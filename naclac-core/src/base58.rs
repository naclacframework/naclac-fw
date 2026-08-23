//! Base58 encoding for on-chain use — the text encoding Solana addresses are
//! conventionally displayed in. Backed by `five8`, the same no-alloc,
//! `no_std` encoder Solana's own `solana-address`/`solana-hash`/
//! `solana-keypair`/`solana-signature` crates use internally.

/// Encodes `bytes` as base58 into `out`, returning the number of bytes
/// written (base58 output length varies with input value, up to 44).
pub fn encode_32(bytes: &[u8; 32], out: &mut [u8; 44]) -> u8 {
    five8::encode_32(bytes, out)
}
