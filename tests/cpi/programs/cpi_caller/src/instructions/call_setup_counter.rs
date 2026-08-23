use naclac_lang::prelude::*;
use cpi_callee_client::{CpiCallee, InitCounterCpi};
use crate::components::CallerAuthority;
use crate::constants::SEED_CALLER_AUTHORITY;

// Real cross-program CPI (unsigned) to a *second* naclac-built program,
// via that program's auto-generated client SDK — the actual mechanism a
// naclac user relies on, not a hand-rolled `invoke` call. Mirrors
// `examples/launchpad`'s `token_creator` -> `amm` CPI exactly
// (`amm_client::{Amm, InitializeCpi}` there, `cpi_callee_client::{CpiCallee,
// InitCounterCpi}` here).
#[derive(Accounts)]
pub struct CallSetupCounter {
    #[account(mut)]
    pub payer: Signer,

    /// SAFETY: this is `cpi_callee`'s own PDA, owned and validated by that
    /// program, not ours — we only ever forward it as a CPI account.
    #[account(mut)]
    pub counter: AccountInfo,

    #[account(seeds = [SEED_CALLER_AUTHORITY], bump)]
    pub caller_authority: Account<CallerAuthority>,

    pub callee_program: Program<CpiCallee>,
    pub system_program: Program<System>,
}

#[instruction]
pub fn call_setup_counter(ctx: Context<CallSetupCounter>) -> Result {
    ctx.accounts.callee_program.init_counter(
        cpi_callee_client::instructions::InitCounterCpiAccounts {
            payer: ctx.accounts.payer.to_cpi_handle_mut(),
            counter: ctx.accounts.counter.to_cpi_handle_mut(),
            authority: ctx.accounts.caller_authority.to_cpi_handle(),
            system_program: ctx.accounts.system_program.to_cpi_handle(),
        },
    )?;
    Ok(())
}
