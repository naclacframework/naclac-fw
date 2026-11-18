//! PDA namespaces and public protocol limits.

pub const SEED_PROTOCOL: &[u8] = b"protocol_v1";
pub const SEED_POOL: &[u8] = b"pool";
pub const SEED_POOL_VAULT: &[u8] = b"pool_vault";
pub const SEED_BET: &[u8] = b"bet";

pub const MAX_STRATEGY_LENGTH: usize = 32;
pub const MERCY_BUFFER_DEFAULT: u64 = 500;
pub const DISCRIMINATOR_SIZE: usize = 8;
pub const MAX_FEE_BPS: u64 = 1000;
pub const MAX_TITLE_LENGTH: usize = 128;
pub const REFUND_TIMEOUT_SECONDS: i64 = 60;
