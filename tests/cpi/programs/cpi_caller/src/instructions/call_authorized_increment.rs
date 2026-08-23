use naclac_lang::prelude::*;
use cpi_callee_client::{CpiCallee, AuthorizedIncrementCpi};
use crate::components::CallerAuthority;
use crate::constants::SEED_CALLER_AUTHORITY;

// Signed cross-program CPI: `cpi_callee`'s `authorized_increment` requires
// its `authority` account to sign, and `Counter.authority` was set (via
// `call_setup_counter`) to this program's own `caller_authority` PDA — a
// PDA can't sign like a real keypair, so this CPI must be `invoke_signed`
// with that PDA's exact seeds, exercising the real
// "signed CPI with PDA seeds" mechanism end to end (not simulated).
#[derive(Accounts)]
pub struct CallAuthorizedIncrement {
    /// SAFETY: `cpi_callee`'s own PDA, validated by that program.
    #[account(mut)]
    pub counter: AccountInfo,

    #[account(seeds = [SEED_CALLER_AUTHORITY], bump)]
    pub caller_authority: Account<CallerAuthority>,

    pub callee_program: Program<CpiCallee>,
}

#[instruction]
pub fn call_authorized_increment(ctx: Context<CallAuthorizedIncrement>) -> Result {
    let bump = ctx.accounts.caller_authority.bump;
    let signer_seeds: &[&[u8]] = &[SEED_CALLER_AUTHORITY, &[bump]];
    let signer: &[&[&[u8]]] = &[signer_seeds];

    ctx.accounts.callee_program.authorized_increment_signed(
        cpi_callee_client::instructions::AuthorizedIncrementCpiAccounts {
            counter: ctx.accounts.counter.to_cpi_handle_mut(),
            authority: ctx.accounts.caller_authority.to_cpi_handle(),
        },
        signer,
    )?;
    Ok(())
}
