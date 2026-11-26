use {
    crate::{errors::EscrowError, events::EscrowExchanged, state::escrow_state::EscrowState},
    naclac_lang::prelude::*,
};

#[derive(Accounts)]
#[instruction(id: u64)]
pub struct Take {
    #[account(mut)]
    pub taker: Signer,

    /// SAFETY: Verified through `escrow_state` PDA seed derivation; receives rent on close.
    #[account(mut)]
    pub maker: UncheckedAccount,

    pub mint_a: Account<Mint>,
    pub mint_b: Account<Mint>,

    #[account(
        mut,
        seeds = EscrowState::seeds(maker.address(), id),
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

pub fn take(ctx: Context<Take>, id: u64) -> Result {
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

    ctx.accounts
        .with_escrow_state_signer(id, ctx.bumps.escrow_state, |accounts, signer| {
            let decimals_a = accounts.mint_a.decimals();
            accounts.token_program.transfer_checked_signed(
                TransferCheckedAccounts {
                    from: &mut accounts.vault_token_account,
                    mint: &accounts.mint_a,
                    to: &mut accounts.taker_token_account_a,
                    authority: &accounts.escrow_state,
                },
                accounts.escrow_state.amount_a,
                decimals_a,
                signer,
            )?;

            accounts.token_program.close_account_signed(
                CloseAccountAccounts {
                    account: &mut accounts.vault_token_account,
                    destination: &mut accounts.maker,
                    authority: &accounts.escrow_state,
                },
                signer,
            )
        })?;

    emit!(EscrowExchanged {
        taker: ctx.accounts.taker.address(),
        amount_a: ctx.accounts.escrow_state.amount_a,
        amount_b: ctx.accounts.escrow_state.amount_b,
    });

    Ok(())
}
