use {
    crate::{errors::EscrowError, events::EscrowCancelled, state::escrow_state::EscrowState},
    naclac_lang::prelude::*,
};

#[derive(Accounts)]
#[instruction(id: u64)]
pub struct Cancel {
    #[account(mut)]
    pub maker: Signer,

    pub mint_a: Account<Mint>,

    #[account(
        mut,
        seeds = EscrowState::seeds(maker.address(), id),
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

pub fn cancel(ctx: Context<Cancel>, id: u64) -> Result {
    ctx.accounts
        .with_escrow_state_signer(id, ctx.bumps.escrow_state, |accounts, signer| {
            let decimals = accounts.mint_a.decimals();
            accounts.token_program.transfer_checked_signed(
                TransferCheckedAccounts {
                    from: &mut accounts.vault_token_account,
                    mint: &accounts.mint_a,
                    to: &mut accounts.maker_token_account_a,
                    authority: &accounts.escrow_state,
                },
                accounts.escrow_state.amount_a,
                decimals,
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

    emit!(EscrowCancelled {
        maker: ctx.accounts.maker.address(),
        amount_a: ctx.accounts.escrow_state.amount_a,
    });

    Ok(())
}
