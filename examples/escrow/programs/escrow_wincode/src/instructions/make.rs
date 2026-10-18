use naclac_lang::prelude::*;
use crate::components::escrow_state::EscrowState;
use crate::constants::SEED_ESCROW;
use crate::errors::EscrowError;
use crate::events::EscrowCreated;

#[derive(Accounts)]
#[instruction(seed: u64)]
pub struct Make {
    #[account(mut)]
    pub maker: Signer,
    pub mint_a: Account<Mint>,
    pub mint_b: Account<Mint>,
    #[account(
        init,
        payer = maker,
        seeds = [SEED_ESCROW, maker.address().as_ref(), &seed.to_le_bytes()],
        bump
    )]
    pub escrow_state: Account<EscrowState>,
    #[account(
        mut,
        token::mint = mint_a,
        token::authority = escrow_state,
    )]
    pub vault_token_account: Account<TokenAccount>,
    #[account(
        mut,
        token::mint = mint_a,
        token::authority = maker,
    )]
    pub maker_token_account_a: Account<TokenAccount>,
    pub token_program: Program<Token>,
    pub system_program: Program<System>,
}
pub fn make(ctx: Context<Make>, _seed: u64, amount_a: u64, amount_b: u64) -> Result {
    require!(amount_a > 0 && amount_b > 0, EscrowError::ZeroAmount);
    require!(ctx.accounts.mint_a.address() != ctx.accounts.mint_b.address(), EscrowError::InvalidMint);

    let escrow = &mut ctx.accounts.escrow_state;
    escrow.maker = ctx.accounts.maker.address();
    escrow.mint_a = ctx.accounts.mint_a.address();
    escrow.mint_b = ctx.accounts.mint_b.address();
    escrow.amount_a = amount_a;
    escrow.amount_b = amount_b;
    escrow.bump = ctx.bumps.escrow_state;
    // Transfer Token A from maker's token account to the vault token account
    let decimals = ctx.accounts.mint_a.decimals();
    ctx.accounts.token_program.transfer_checked(
        TransferCheckedAccounts {
            from: &mut ctx.accounts.maker_token_account_a,
            mint: &ctx.accounts.mint_a,
            to: &mut ctx.accounts.vault_token_account,
            authority: &ctx.accounts.maker,
        },
        amount_a,
        decimals,
    )?;
    emit!(EscrowCreated {
        maker: ctx.accounts.maker.address(),
        amount_a,
        amount_b,
    });
    Ok(())
}