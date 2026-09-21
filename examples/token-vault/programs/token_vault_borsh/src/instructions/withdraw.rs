use naclac_lang::prelude::*;
use crate::components::vault::Vault;
use crate::components::user_account::UserAccount;
use crate::constants::{SEED_VAULT, SEED_USER_ACCOUNT};
use crate::events::TokenWithdrawn;
use crate::systems::vault_ops::process_withdraw;

#[derive(Accounts)]
#[instruction(vault_id: u64)]
pub struct Withdraw {
    #[account(mut)]
    pub payer: Signer,

    pub mint: InterfaceAccount<Mint>,

    #[account(
        mut,
        seeds = [SEED_VAULT, &vault_id.to_le_bytes()],
        bump = vault_account.bump
    )]
    pub vault_account: Account<Vault>,

    #[account(mut)]
    pub vault_token_account: Account<TokenAccount>,

    #[account(mut)]
    pub user_token_account: Account<TokenAccount>,

    #[account(
        mut,
        seeds = [SEED_USER_ACCOUNT, &vault_id.to_le_bytes(), payer.address(), mint.address()],
        bump = user_account.bump
    )]
    pub user_account: Account<UserAccount>,

    pub token_program: Interface<TokenInterface>,
}

pub fn withdraw(ctx: Context<Withdraw>, vault_id: u64, amount: u64) -> Result {
    let vault_account_addr = ctx.accounts.vault_account.address();
    let mint_addr = ctx.accounts.mint.address();

    // Verify token account mints
    require!(
        ctx.accounts.vault_token_account.mint() == mint_addr,
        crate::errors::VaultError::InvalidMint
    );
    require!(
        ctx.accounts.user_token_account.mint() == mint_addr,
        crate::errors::VaultError::InvalidMint
    );

    // Verify vault_token_account owner is the vault PDA
    require!(
        ctx.accounts.vault_token_account.owner() == vault_account_addr,
        crate::errors::VaultError::Unauthorized
    );

    let user = &mut ctx.accounts.user_account;
    process_withdraw(user, amount)?;

    let vault_id_bytes = vault_id.to_le_bytes();
    let seeds: &[&[u8]] = &[
        SEED_VAULT,
        &vault_id_bytes,
        &[ctx.accounts.vault_account.bump],
    ];
    let signer_seeds: &[&[&[u8]]] = &[seeds];

    let decimals = ctx.accounts.mint.decimals();

    ctx.accounts.token_program.transfer_checked_signed(
        TransferCheckedAccounts {
            from: &mut ctx.accounts.vault_token_account,
            mint: &ctx.accounts.mint,
            to: &mut ctx.accounts.user_token_account,
            authority: &ctx.accounts.vault_account,
        },
        amount,
        decimals,
        signer_seeds,
    )?;

    emit!(TokenWithdrawn {
        user: ctx.accounts.payer.address(),
        amount,
        total_vault_balance: ctx.accounts.vault_token_account.amount() - amount,
    });

    Ok(())
}
