// ===========================================================================
// extensions/cpi_guard.rs — CpiGuard
// ===========================================================================

//! `CpiGuard` (token-account extension) — read side only. Verified against
//! the real `spl-token-2022-11.0.0` processor: `Enable`/`Disable` both call
//! `in_cpi()` (checks `sol_get_stack_height() > TRANSACTION_LEVEL_STACK_HEIGHT`)
//! and reject with `CpiGuardSettingsLocked` whenever Token-2022 is invoked
//! via CPI at all — not just from an unexpected caller. A naclac-token CPI
//! helper for either action would therefore always fail when actually used
//! the way every other CPI helper in this crate is used (called from inside
//! an on-chain program's own instruction, which is itself a CPI into
//! Token-2022) — toggling this extension is a client/wallet-level action,
//! never an on-chain program's. The official `anchor-spl` crate ships
//! `cpi_guard_enable`/`cpi_guard_disable` anyway with no such warning; the
//! local pinocchio-native `anchor-spl-v2` instead `#[deprecated]`s and
//! panics both. naclac-token does neither — it only implements what can
//! genuinely be used from on-chain code: reading whether the extension is
//! present, which a program might reasonably want to check.

use super::Extension;
use crate::prelude::{Pod, Zeroable};

/// Raw Token-2022 `CpiGuard` token-account extension (1 byte): whether
/// certain operations (`Transfer`/`Burn` without a delegate, `Approve`,
/// unrestricted `SetAuthority`, `CloseAccount` to a non-owner) are blocked
/// when attempted via CPI against this account. Layout verified against
/// `spl-token-2022-interface::extension::cpi_guard::CpiGuard`.
#[repr(transparent)]
#[derive(Copy, Clone)]
pub struct CpiGuard(pub [u8; 1]);

unsafe impl Pod for CpiGuard {}
unsafe impl Zeroable for CpiGuard {}
impl Extension for CpiGuard {
    const TYPE: u16 = 11; // ExtensionType::CpiGuard
    const ACCOUNT_TYPE: u8 = 2; // AccountType::Account
}

impl CpiGuard {
    /// Raw `bool`-as-`u8` flag: whether CPI-issued privileged operations
    /// are currently locked on this account.
    pub fn lock_cpi(&self) -> bool {
        self.0[0] != 0
    }
}
