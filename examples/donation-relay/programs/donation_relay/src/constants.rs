use naclac_lang::prelude::*;

pub const EPOCH_TRACKER_V1_SEED: &[u8] = b"epoch_tracker_v1";
pub const DEBOUNCER_V1_SEED: &[u8] = b"debouncer_v1";
pub const MINT_WHITELIST_V1_SEED: &[u8] = b"mint_whitelist_v1";
pub const WSOL_MINT: Address = address!("So11111111111111111111111111111111111111112");

/// Real on-chain limit for `message`, confirmed byte-exact via
/// `reference/donation-relay-probe/src/bin/probe3.rs`'s binary search
/// (255 succeeds, 256 fails with `InvalidMessageLength`).
pub const MAX_MESSAGE_LEN: usize = 255;
