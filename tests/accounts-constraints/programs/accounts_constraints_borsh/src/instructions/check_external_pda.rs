use naclac_lang::prelude::*;
use crate::constants::SEED_EXTERNAL_PDA;

// `seeds::program`: derives/validates a PDA against a *different* program's
// ID than the current program — for cases like validating a PDA owned by an
// external program (`naclac-macros/src/instruction/parser.rs`'s
// `pda_program`, consumed in `security.rs`'s PDA derivation as
// `pda_program_tokens`). Uses the real, well-known System Program ID as the
// "external" program — a real, stable address that needs no separate
// deployment to exercise this.
//
// Explicit `bump = bump` (not bare `bump`): a bare `AccountInfo` has no
// stored `.bump` field for the bare-bump auto-path to read (see
// `touch_seeded.rs` for the same explicit-bump pattern on `Account<T>`).
#[derive(Accounts)]
#[instruction(bump: u8)]
pub struct CheckExternalPda {
    /// SAFETY: only this account's own address is compared against the PDA
    /// derived from `SEED_EXTERNAL_PDA` + `bump` against the System
    /// Program's ID; its data is never read or deserialized.
    #[account(
        seeds = [SEED_EXTERNAL_PDA],
        bump = bump,
        seeds::program = naclac_lang::prelude::SYSTEM_PROGRAM_ID
    )]
    pub target: AccountInfo,
}

pub fn check_external_pda(_ctx: Context<CheckExternalPda>, _bump: u8) -> Result {
    Ok(())
}
