use naclac_lang::prelude::*;

// Mirror of the real `EpochTrackerV1` account. Real `current_epoch` is `u128`,
// but naclac's `#[component]` zero-copy macro rejects fields needing more
// than 8-byte alignment (see naclac-macros/src/component.rs). This scoped
// pass only ever creates a fresh tracker (confirmed always `current_epoch = 0`
// via `reference/donation-relay-probe/src/bin/probe1.rs`) and never
// increments it — `close_donation_epoch_v1` (out of scope) would be the
// instruction that needs real epoch arithmetic — so `u64` is sufficient here.
#[component]
pub struct EpochTracker {
    pub bump: u8,
    // 0 = Uninitialized, 1 = Initialized (real type is an enum; bytemuck::Pod
    // can't be derived for enums, so this stays a plain u8).
    pub state: u8,
    pub config_id: Address,
    pub mint: Address,
    pub current_epoch: u64,
}
