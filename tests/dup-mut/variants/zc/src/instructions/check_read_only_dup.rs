//! Exercises the duplicate rules of read-only fields: `repeat` and
//! `program_repeat` may repeat a read-only account, `unmarked` may not, and
//! no field may repeat `written`.

use naclac_lang::prelude::*;

#[derive(Accounts)]
pub struct CheckReadOnlyDup {
    /// SAFETY: never read or written; only its slot and flags matter.
    #[account(mut)]
    pub written: AccountInfo,

    /// SAFETY: never read; only its slot matters.
    pub plain: AccountInfo,

    /// SAFETY: never read; only its slot matters.
    #[account(dup)]
    pub repeat: AccountInfo,

    /// SAFETY: never read; only its slot and flags matter.
    #[account(dup, executable)]
    pub program_repeat: AccountInfo,

    /// SAFETY: never read; only its slot matters.
    pub unmarked: AccountInfo,
}

pub fn check_read_only_dup(_ctx: Context<CheckReadOnlyDup>) -> Result {
    Ok(())
}
