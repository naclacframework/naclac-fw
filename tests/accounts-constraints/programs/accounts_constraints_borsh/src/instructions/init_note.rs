use naclac_lang::prelude::*;
use crate::components::Note;
use crate::constants::SEED_NOTE;

/// Real end-to-end proof that the default `init` space computation for a
/// Borsh `#[component]` uses `Note::SPACE` (the real, `#[max_len]`-aware
/// serialized size) rather than `size_of::<Note>()` (the much smaller
/// in-memory `String` pointer/len/cap representation) — see
/// `naclac-macros/docs/derive-accounts-gaps-audit.md`'s gap #3. No
/// `space =` is given here deliberately, so this only compiles/runs
/// correctly if the macro's default is the real serialized size.
#[derive(Accounts)]
pub struct InitNote {
    #[account(mut)]
    pub payer: Signer,

    #[account(
        init,
        payer = payer,
        seeds = [SEED_NOTE],
        bump
    )]
    pub note: Account<Note>,

    pub system_program: Program<System>,
}

#[instruction]
pub fn init_note(ctx: Context<InitNote>, name: String) -> Result {
    ctx.accounts.note.name = name;
    Ok(())
}
