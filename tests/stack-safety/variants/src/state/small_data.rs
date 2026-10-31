use naclac_lang::prelude::*;

/// Well under both the 300-byte per-field and 1700-byte aggregate stack
/// budgets — used unboxed, to confirm the ordinary path needs no boxing.
#[account]
pub struct SmallData {
    pub bump: u8,
    pub value: u64,
}
