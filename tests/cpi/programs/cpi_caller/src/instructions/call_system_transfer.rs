use naclac_lang::prelude::*;
use crate::components::CallerAuthority;
use crate::constants::SEED_CALLER_AUTHORITY;

// Real CPI to the System Program — `SystemTransferAccounts`, the
// `naclac-core/src/system_program.rs` helper.
#[derive(Accounts)]
pub struct CallSystemTransfer {
    #[account(mut)]
    pub payer: Signer,

    #[account(mut, seeds = [SEED_CALLER_AUTHORITY], bump)]
    pub caller_authority: Account<CallerAuthority>,

    pub system_program: Program<System>,
}

#[instruction]
pub fn call_system_transfer(ctx: Context<CallSystemTransfer>, amount: u64) -> Result {
    ctx.accounts.system_program.transfer(
        SystemTransferAccounts {
            from: &mut ctx.accounts.payer,
            to: &mut ctx.accounts.caller_authority,
        },
        amount,
    )?;
    Ok(())
}
