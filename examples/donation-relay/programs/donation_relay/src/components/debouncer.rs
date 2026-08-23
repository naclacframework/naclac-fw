use naclac_lang::prelude::*;

// Mirror of the real `DebouncerV1` account. `total_amount` accumulates
// across repeated donations to the same `(config_id, mint)` pair — confirmed
// via `reference/donation-relay-probe/src/bin/probe4.rs`.
#[component]
pub struct Debouncer {
    pub bump: u8,
    // 0 = Uninitialized, 1 = Initialized (real type is an enum; bytemuck::Pod
    // can't be derived for enums, so this stays a plain u8).
    pub state: u8,
    pub config_id: Address,
    pub mint: Address,
    pub total_amount: u64,
}
