use crate::components::escrow_state::EscrowState;
use crate::constants::SEED_ESCROW;
use crate::errors::EscrowError;
use crate::events::EscrowCancelled;
use naclac_lang::prelude::*;

#[derive(Accounts)]
#[instruction(seed: u64)]
pub struct Cancel {
    #[account(mut)]
    pub maker: Signer,

    pub mint_a: Account<Mint>,

    #[account(
        mut,
        seeds = [SEED_ESCROW, maker.address().as_ref(), &seed.to_le_bytes()],
        bump = escrow_state.bump,
        close = maker,
        mint_a = mint_a @ EscrowError::InvalidMint,
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

pub fn cancel(ctx: Context<Cancel>, seed: u64) -> Result {

    // 1. Prepare signer seeds for the escrow state PDA
    let maker_addr = ctx.accounts.maker.address();
    let seed_bytes = seed.to_le_bytes();
    let seeds: &[&[u8]] = &[
        SEED_ESCROW,
        maker_addr.as_ref(),
        &seed_bytes,
        &[ctx.accounts.escrow_state.bump],
    ];
    let signer_seeds: &[&[&[u8]]] = &[seeds];

    // 2. Refund Token A from the vault token account back to the Maker
    let decimals = ctx.accounts.mint_a.decimals();
    ctx.accounts.token_program.transfer_checked_signed(
        TransferCheckedAccounts {
            from: &mut ctx.accounts.vault_token_account,
            mint: &ctx.accounts.mint_a,
            to: &mut ctx.accounts.maker_token_account_a,
            authority: &ctx.accounts.escrow_state,
        },
        ctx.accounts.escrow_state.amount_a,
        decimals,
        signer_seeds,
    )?;

    // 3. Close the vault token account
    ctx.accounts.token_program.close_account_signed(
        CloseAccountAccounts {
            account: &mut ctx.accounts.vault_token_account,
            destination: &mut ctx.accounts.maker,
            authority: &ctx.accounts.escrow_state,
        },
        signer_seeds,
    )?;

    emit!(EscrowCancelled {
        maker: ctx.accounts.maker.address(),
        amount_a: ctx.accounts.escrow_state.amount_a,
    });

    Ok(())
}
