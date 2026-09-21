//! Base58 encoding for on-chain use — the text encoding Solana addresses are
//! conventionally displayed in. Backed by `five8`, the same no-alloc,
//! `no_std` encoder Solana's own `solana-address`/`solana-hash`/
//! `solana-keypair`/`solana-signature` crates use internally.

/// Encodes `bytes` as base58 into `out`, returning the number of bytes
/// written (base58 output length varies with input value, up to 44).
pub fn encode_32(bytes: &[u8; 32], out: &mut [u8; 44]) -> u8 {
    five8::encode_32(bytes, out)
}

/// `encode_32`, returning an owned `String` — `idl-build`-only since it
/// needs `std`/`alloc`; `naclac_core::prelude::Address` has no `Display`
/// impl of its own (unlike `solana_program::pubkey::Pubkey`), so this is
/// the actual conversion path for anything printing an address as text —
/// the same encoder every on-chain-visible address representation
/// ultimately uses.
#[cfg(feature = "idl-build")]
pub fn encode_32_to_string(bytes: &[u8; 32]) -> std::string::String {
    let mut buf = [0u8; 44];
    let len = encode_32(bytes, &mut buf);
    std::str::from_utf8(&buf[..len as usize])
        .expect("naclac base58 encoder always produces valid ASCII")
        .to_string()
}

/// `encode_32_to_string` for any address type reachable only via
/// `AsRef<[u8]>` — the one accessor `prelude::Address` (pinocchio backend,
/// public `.0`) and `solana_address::Address` (solana backend, private
/// inner field) both actually implement, so idl-build codegen never needs
/// to know which backend produced the value it's encoding.
#[cfg(feature = "idl-build")]
pub fn encode_address_to_string<A: AsRef<[u8]>>(addr: &A) -> std::string::String {
    let bytes: [u8; 32] = addr
        .as_ref()
        .try_into()
        .expect("naclac address types are always exactly 32 bytes");
    encode_32_to_string(&bytes)
}
