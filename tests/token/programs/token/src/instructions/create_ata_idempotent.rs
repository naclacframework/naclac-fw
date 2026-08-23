use naclac_lang::prelude::*;

#[derive(Accounts)]
pub struct CreateAssociatedTokenAccountIdempotent {
    #[account(mut)]
    pub payer: Signer,

    /// SAFETY: only used as the ATA's `authority` argument to the
    /// Associated Token Program CPI below; never read or deserialized.
    pub owner: AccountInfo,

    pub mint: Account<Mint>,

    /// SAFETY: may not exist yet — created idempotently by the CPI below
    /// (a no-op if it already exists).
    #[account(
        mut,
        init_if_needed,
        payer = payer,
        associated_token::mint = mint,
        associated_token::authority = owner,
    )]
    pub associated_token: AccountInfo,

    pub system_program: Program<System>,
    pub token_program: Program<Token>,
    pub associated_token_program: Program<AssociatedToken>,
}

#[instruction]
pub fn create_ata_idempotent(_ctx: Context<CreateAssociatedTokenAccountIdempotent>) -> Result {
    Ok(())
}
