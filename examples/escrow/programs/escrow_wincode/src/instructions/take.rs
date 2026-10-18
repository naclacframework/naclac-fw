use naclac_lang::prelude::*;
use crate::components::escrow_state::EscrowState;
use crate::constants::SEED_ESCROW;
use crate::errors::EscrowError;
use crate::events::EscrowExchanged;

#[derive(Accounts)]
#[instruction(seed: u64)]
pub struct Take {
    #[account(mut)]
    pub taker: Signer,

    /// SAFETY: Verified through `escrow_state` PDA seed derivation; receives rent on close.
    #[account(mut)]
    pub maker: AccountInfo,

    pub mint_a: Account<Mint>,
    pub mint_b: Account<Mint>,

    #[account(
        mut,
        seeds = [SEED_ESCROW, maker.address().as_ref(), &seed.to_le_bytes()],
        bump = escrow_state.bump,
        close = maker,
        mint_a = mint_a @ EscrowError::InvalidMint,
        mint_b = mint_b @ EscrowError::InvalidMint,
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
        token::authority = taker,
    )]
    pub taker_token_account_a: Account<TokenAccount>,

    #[account(
        mut,
        token::mint = mint_b,
        token::authority = taker,
    )]
    pub taker_token_account_b: Account<TokenAccount>,

    #[account(
        mut,
        token::mint = mint_b,
        token::authority = maker,
    )]
    pub maker_token_account_b: Account<TokenAccount>,

    pub token_program: Program<Token>,
    pub system_program: Program<System>,
}

pub fn take(ctx: Context<Take>, seed: u64) -> Result {

    // 1. Taker transfers Token B directly to the Maker
    let decimals_b = ctx.accounts.mint_b.decimals();
    ctx.accounts.token_program.transfer_checked(
        TransferCheckedAccounts {
            from: &mut ctx.accounts.taker_token_account_b,
            mint: &ctx.accounts.mint_b,
            to: &mut ctx.accounts.maker_token_account_b,
            authority: &ctx.accounts.taker,
        },
        ctx.accounts.escrow_state.amount_b,
        decimals_b,
    )?;

    // 2. Prepare signer seeds for the escrow state PDA
    let maker_addr = ctx.accounts.maker.address();
    let seed_bytes = seed.to_le_bytes();
    let seeds: &[&[u8]] = &[
        SEED_ESCROW,
        maker_addr.as_ref(),
        &seed_bytes,
        &[ctx.accounts.escrow_state.bump],
    ];
    let signer_seeds: &[&[&[u8]]] = &[seeds];

    // 3. Transfer Token A from the vault token account to the Taker
    let decimals_a = ctx.accounts.mint_a.decimals();
    ctx.accounts.token_program.transfer_checked_signed(
        TransferCheckedAccounts {
            from: &mut ctx.accounts.vault_token_account,
            mint: &ctx.accounts.mint_a,
            to: &mut ctx.accounts.taker_token_account_a,
            authority: &ctx.accounts.escrow_state,
        },
        ctx.accounts.escrow_state.amount_a,
        decimals_a,
        signer_seeds,
    )?;

    // 4. Close the vault token account (destinations receive refunded lamports)
    ctx.accounts.token_program.close_account_signed(
        CloseAccountAccounts {
            account: &mut ctx.accounts.vault_token_account,
            destination: &mut ctx.accounts.maker,
            authority: &ctx.accounts.escrow_state,
        },
        signer_seeds,
    )?;

    emit!(EscrowExchanged {
        taker: ctx.accounts.taker.address(),
        amount_a: ctx.accounts.escrow_state.amount_a,
        amount_b: ctx.accounts.escrow_state.amount_b,
    });

    Ok(())
}
