use naclac_lang::prelude::*;
use crate::constants::MAX_SHAREHOLDERS;

#[naclac_pod]
pub struct Shareholder {
    pub address: Address,
    pub share_bps: u16,
}

// status: 0 = Paused, 1 = Active (real type is a ConfigStatus enum; bytemuck::Pod
// can't be derived for enums, so this stays a plain u8).
pub const SHARING_CONFIG_STATUS_PAUSED: u8 = 0;
pub const SHARING_CONFIG_STATUS_ACTIVE: u8 = 1;

// Field order matters here and must match the real account exactly: real
// `pump_fees::SharingConfig` Borsh-encodes `shareholders` as a `Vec<Shareholder>`,
// whose 4-byte length prefix comes immediately before the entries -- so
// `shareholders_len` is declared before `shareholders` below, not after.
// Confirmed against real deployed bytecode (`reference/fee-tier-probe/src/bin/probe70.rs`):
// a freshly-created real account is exactly 1024 bytes, `shareholders_len`
// sits right after `admin_revoked`, and everything past the actual
// shareholder entries is zero-padding, not further structured data --
// `[Shareholder; MAX_SHAREHOLDERS]` with only `shareholders[..shareholders_len]`
// ever read or written correctly reproduces that.
#[component]
pub struct SharingConfig {
    pub bump: u8,
    pub version: u8,
    pub status: u8,
    pub mint: Address,
    pub admin: Address,
    pub admin_revoked: u8,
    pub shareholders_len: u32,
    pub shareholders: [Shareholder; MAX_SHAREHOLDERS],
}
