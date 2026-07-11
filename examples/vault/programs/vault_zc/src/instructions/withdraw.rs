use naclac_lang::prelude::*;
use crate::components::vault::Vault;
use crate::components::user_account::UserAccount;
use crate::constants::SEED_USER_ACCOUNT;
use crate::systems::vault_ops::process_withdraw;
use crate::events::FundsWithdrawn;

#[derive(Accounts)]
pub struct Withdraw {
    #[account(mut)]
    pub user: Signer,

    #[account(
        mut,
        seeds = [SEED_VAULT]
    )]
    pub vault_account: Account<Vault>,

    #[account(
        mut,
        seeds = [SEED_USER_ACCOUNT, user],
        bump = user_account.bump
    )]
    pub user_account: Account<UserAccount>,

    pub system_program: Program<System>,
}

#[instruction]
pub fn withdraw(ctx: Context<Withdraw>, amount: u64) -> Result {
    // Update balances
    process_withdraw(&mut ctx.accounts.vault_account, &mut ctx.accounts.user_account, amount)?;

    // Transfer lamports from vault (PDA) to user by directly modifying lamport balances
    ctx.accounts.vault_account.sub_lamports(amount)?;
    ctx.accounts.user.add_lamports(amount)?;

    emit!(FundsWithdrawn {
        user: ctx.accounts.user.address(),
        amount,
        total_vault_balance: ctx.accounts.vault_account.total_deposited,
    });

    Ok(())
}
