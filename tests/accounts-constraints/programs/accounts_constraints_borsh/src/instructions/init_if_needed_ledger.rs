use naclac_lang::prelude::*;
use crate::components::Ledger;
use crate::constants::SEED_LEDGER;

// `init_if_needed`: the framework skips the account-creation CPI on a
// second call against the same PDA (it doesn't re-run `init` or reset the
// account). The framework gives the handler no explicit "was this call the
// one that created it" flag, so — same as any real naclac program author
// would — we use a sentinel check (`value == 0` means never set) to make the
// handler itself idempotent. If `init_if_needed` ever silently wiped the
// account's existing data on a second call, this sentinel would incorrectly
// see `0` again and overwrite it, and the test would catch that.
#[derive(Accounts)]
pub struct InitIfNeededLedger {
    #[account(mut)]
    pub payer: Signer,

    #[account(
        init_if_needed,
        payer = payer,
        seeds = [SEED_LEDGER],
        bump
    )]
    pub ledger: Account<Ledger>,

    pub system_program: Program<System>,
}

#[instruction]
pub fn init_if_needed_ledger(ctx: Context<InitIfNeededLedger>, value: u64) -> Result {
    let ledger = &mut ctx.accounts.ledger;
    if ledger.value == 0 {
        ledger.value = value;
    }
    Ok(())
}
