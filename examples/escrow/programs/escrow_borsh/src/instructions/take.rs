use naclac_lang::prelude::*;
use crate::components::escrow_state::EscrowState;
use crate::constants::SEED_ESCROW;
use crate::events::EscrowExchanged;

#[derive(Accounts)]
#[instruction(seed: u64)]
pub struct Take {
    #[account(mut)]
    pub taker: Signer,

    #[account(mut)]
    pub maker: AccountInfo,

    pub mint_a: Account<Mint>,
    pub mint_b: Account<Mint>,

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
    pub taker_token_account_a: Account<TokenAccount>,

    #[account(mut)]
    pub taker_token_account_b: Account<TokenAccount>,

    #[account(mut)]
    pub maker_token_account_b: Account<TokenAccount>,

    pub token_program: Program<Token>,
    pub system_program: Program<System>,
}

#[instruction]
pub fn take(ctx: Context<Take>, seed: u64) -> Result {
    // Verify vault token account mint and owner
    require!(
        ctx.accounts.vault_token_account.mint() == ctx.accounts.mint_a.address(),
        crate::errors::EscrowError::InvalidMint
    );
    require!(
        ctx.accounts.mint_a.address() == ctx.accounts.escrow_state.mint_a,
        crate::errors::EscrowError::InvalidMint
    );
    require!(
        ctx.accounts.mint_b.address() == ctx.accounts.escrow_state.mint_b,
        crate::errors::EscrowError::InvalidMint
    );
    require!(
        ctx.accounts.vault_token_account.owner() == ctx.accounts.escrow_state.address(),
        crate::errors::EscrowError::Unauthorized
    );
    require!(
        ctx.accounts.taker_token_account_a.owner() == ctx.accounts.taker.address(),
        crate::errors::EscrowError::Unauthorized
    );
    require!(
        ctx.accounts.taker_token_account_a.mint() == ctx.accounts.escrow_state.mint_a,
        crate::errors::EscrowError::InvalidMint
    );
    require!(
        ctx.accounts.taker_token_account_b.owner() == ctx.accounts.taker.address(),
        crate::errors::EscrowError::Unauthorized
    );
    require!(
        ctx.accounts.taker_token_account_b.mint() == ctx.accounts.escrow_state.mint_b,
        crate::errors::EscrowError::InvalidMint
    );
    require!(
        ctx.accounts.maker_token_account_b.owner() == ctx.accounts.escrow_state.maker,
        crate::errors::EscrowError::Unauthorized
    );
    require!(
        ctx.accounts.maker_token_account_b.mint() == ctx.accounts.escrow_state.mint_b,
        crate::errors::EscrowError::InvalidMint
    );

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
