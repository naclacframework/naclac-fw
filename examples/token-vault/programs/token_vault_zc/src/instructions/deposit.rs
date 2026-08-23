use naclac_lang::prelude::*;
use crate::components::vault::Vault;
use crate::components::user_account::UserAccount;
use crate::constants::SEED_USER_ACCOUNT;
use crate::constants::SEED_VAULT;
use crate::events::TokenDeposited;
use crate::systems::vault_ops::process_deposit;

#[derive(Accounts)]
#[instruction(vault_id: u64, _amount: u64, user_bump: u8)]
pub struct Deposit {
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
        init_if_needed,
        payer = payer,
        seeds = [SEED_USER_ACCOUNT, &vault_id.to_le_bytes(), payer.address(), mint.address()],
        bump = user_bump
    )]
    pub user_account: Account<UserAccount>,

    pub token_program: Interface<TokenInterface>,

    pub system_program: Program<System>,
}

#[instruction]
pub fn deposit(ctx: Context<Deposit>, _vault_id: u64, amount: u64, user_bump: u8) -> Result {
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

    let decimals = ctx.accounts.mint.decimals();

    ctx.accounts.token_program.transfer_checked(
        TransferCheckedAccounts {
            from: &mut ctx.accounts.user_token_account,
            mint: &ctx.accounts.mint,
            to: &mut ctx.accounts.vault_token_account,
            authority: &ctx.accounts.payer,
        },
        amount,
        decimals,
    )?;

    let user = &mut ctx.accounts.user_account;
    process_deposit(user, amount)?;

    if user.owner == Address::default() {
        user.owner = ctx.accounts.payer.address();
        user.bump = user_bump;
    }

    emit!(TokenDeposited {
        user: ctx.accounts.payer.address(),
        amount,
        total_vault_balance: ctx.accounts.vault_token_account.amount() + amount,
    });

    Ok(())
}
