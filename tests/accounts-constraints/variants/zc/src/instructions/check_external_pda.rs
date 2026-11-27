use naclac_lang::prelude::*;
use crate::constants::SEED_EXTERNAL_PDA;

// `seeds::program`: derives/validates a PDA against a *different* program's
// ID than the current program — for cases like validating a PDA owned by an
// external program (`derive/src/accounts/field/parser`'s
// `pda_program`, consumed in `security.rs`'s PDA derivation as
// `pda_program_tokens`). Uses the real, well-known System Program ID as the
// "external" program — a real, stable address that needs no separate
// deployment to exercise this.
//
// `authority` is one of the seeds, so the seeds are not all constants and
// the explicit `bump = bump` is used as given: one hash and a compare, with
// no proof that the bump is canonical.
#[derive(Accounts)]
#[instruction(bump: u8)]
pub struct CheckExternalPda {
    /// SAFETY: only this account's own address is compared against the PDA
    /// derived from `SEED_EXTERNAL_PDA`, `authority` and `bump` against the
    /// System Program's ID; its data is never read or deserialized.
    #[account(
        seeds = [SEED_EXTERNAL_PDA, authority.address().as_ref()],
        bump = bump,
        seeds::program = naclac_lang::prelude::SYSTEM_PROGRAM_ID
    )]
    pub target: UncheckedAccount,

    /// SAFETY: only its address is used as a seed; never read or deserialized.
    pub authority: UncheckedAccount,
}

pub fn check_external_pda(_ctx: Context<CheckExternalPda>, _bump: u8) -> Result {
    Ok(())
}
