use crate::components::escrow_state::EscrowState;
use crate::constants::SEED_ESCROW;
use crate::events::EscrowCancelled;
use naclac_lang::prelude::*;

#[derive(Accounts)]
#[instruction(seed: u64)]
pub struct Cancel {
    #[account(mut)]
    pub maker: Signer,

    pub mint_a: AccountInfo,

    #[account(
        mut,
        seeds = [SEED_ESCROW, maker.address().as_ref(), &seed.to_le_bytes()],
        bump = escrow_state.bump,
        close = maker
    )]
    pub escrow_state: Account<EscrowState>,

    #[account(mut)]
    pub vault_token_account: Account<TokenAccount>,

    #[account(mut)]
    pub maker_token_account_a: Account<TokenAccount>,

    pub token_program: Program<Token>,
    pub system_program: Program<System>,
}

#[instruction]
pub fn cancel(ctx: Context<Cancel>, seed: u64) -> Result {
    require!(
        ctx.accounts.vault_token_account.mint() == ctx.accounts.mint_a.address(),
        crate::errors::EscrowError::InvalidMint
    );
    require!(
        ctx.accounts.mint_a.address() == ctx.accounts.escrow_state.mint_a,
        crate::errors::EscrowError::InvalidMint
    );
    require!(
        ctx.accounts.vault_token_account.owner() == ctx.accounts.escrow_state.address(),
        crate::errors::EscrowError::Unauthorized
    );
    require!(
        ctx.accounts.maker_token_account_a.owner() == ctx.accounts.maker.address(),
        crate::errors::EscrowError::Unauthorized
    );
    require!(
        ctx.accounts.maker_token_account_a.mint() == ctx.accounts.escrow_state.mint_a,
        crate::errors::EscrowError::InvalidMint
    );

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
    ctx.accounts.token_program.transfer_signed(
        &ctx.accounts.vault_token_account,
        &ctx.accounts.maker_token_account_a,
        &ctx.accounts.escrow_state,
        ctx.accounts.escrow_state.amount_a,
        signer_seeds,
    )?;

    // 3. Close the vault token account
    ctx.accounts.token_program.close_account_signed(
        &ctx.accounts.vault_token_account,
        &ctx.accounts.maker,
        &ctx.accounts.escrow_state,
        signer_seeds,
    )?;

    emit!(EscrowCancelled {
        maker: ctx.accounts.maker.address(),
        amount_a: ctx.accounts.escrow_state.amount_a,
    });

    Ok(())
}
