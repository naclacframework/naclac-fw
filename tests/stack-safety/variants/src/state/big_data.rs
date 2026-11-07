use naclac_lang::prelude::*;

/// The 300-byte payload alone puts `Account<BigData>` over the per-field
/// stack budget regardless of `AccountInfo`'s own overhead — every field of
/// this type in this program must be `Box<Account<BigData>>`, or the whole
/// crate fails to compile (`derive/src/accounts/stack_guard.rs`).
#[account]
pub struct BigData {
    pub bump: u8,
    pub payload: [u8; 300],
}
