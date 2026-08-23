use naclac_lang::prelude::*;

// Real on-chain size is 600 bytes; the documented IDL fields below only sum
// to 544 (536 + 8 disc) — the 56-byte gap is confirmed real (probe-observed
// `ConstraintSpace` mismatch), content not identified.
#[component]
pub struct GlobalVolumeAccumulator {
    pub start_time: i64,
    pub end_time: i64,
    pub seconds_in_a_day: i64,
    pub mint: Address,
    pub total_token_supply: [u64; 30],
    pub sol_volumes: [u64; 30],
    pub _reserved_trailing: [u8; 56],
}
