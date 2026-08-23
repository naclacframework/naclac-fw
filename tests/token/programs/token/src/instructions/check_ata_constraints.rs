use naclac_lang::prelude::*;

// `associated_token::mint`/`associated_token::authority`/`associated_token::bump`
// on an *existing* (non-`init`) associated token account — exercises the
// PDA hash-and-compare check in `check_associated_token_address`
// (`naclac-token/src/associated_token.rs`), the fix for the gap documented
// in `naclac-token/docs/04-associated-token-existing-account-gap.md`.
#[derive(Accounts)]
#[instruction(ata_bump: u8)]
pub struct CheckAtaConstraints {
    pub mint: Account<Mint>,

    /// SAFETY: only used as the ATA's `authority` for constraint
    /// verification below; never read or deserialized.
    pub owner: AccountInfo,

    #[account(
        associated_token::mint = mint,
        associated_token::authority = owner,
        associated_token::bump = ata_bump,
    )]
    pub associated_token: Account<TokenAccount>,

    pub token_program: Program<Token>,
}

#[instruction]
pub fn check_ata_constraints(_ctx: Context<CheckAtaConstraints>, _ata_bump: u8) -> Result {
    Ok(())
}
