use naclac_lang::prelude::*;
use crate::components::vault::Vault;
use crate::components::user_account::UserAccount;
use crate::constants::{SEED_USER_ACCOUNT};
use crate::systems::vault_ops::process_deposit;
use crate::events::FundsDeposited;

#[derive(Accounts)]
#[instruction(_amount: u64, user_bump: u8)]
pub struct Deposit {
    #[account(mut)]
    pub payer: Signer,

    #[account(
        mut,
        seeds = [SEED_VAULT]
    )]
    pub vault_account: Account<Vault>,

    #[account(
        init_if_needed,
        payer = payer,
        seeds = [SEED_USER_ACCOUNT, payer],
        bump = user_bump
    )]
    pub user_account: Account<UserAccount>,

    pub system_program: Program<System>,
}

pub fn deposit(ctx: Context<Deposit>, amount: u64, user_bump: u8) -> Result {
    // Transfer lamports from payer to vault
    ctx.accounts.system_program.transfer(
        SystemTransferAccounts {
            from: &mut ctx.accounts.payer,
            to: &mut ctx.accounts.vault_account,
        },
        amount,
    )?;

    let vault = &mut ctx.accounts.vault_account;
    let user = &mut ctx.accounts.user_account;

    // Update balances
    process_deposit(vault, user, amount)?;

    // Initialize owner and bump if new account
    if user.owner == Address::default() {
        user.owner = ctx.accounts.payer.address();
        user.bump = user_bump;
    }

    emit!(FundsDeposited {
        user: ctx.accounts.payer.address(),
        amount,
        total_vault_balance: vault.total_deposited,
    });

    Ok(())
}
