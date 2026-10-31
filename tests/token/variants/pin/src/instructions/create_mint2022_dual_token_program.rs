use naclac_lang::prelude::*;
use crate::state::MintAuthority;
use crate::constants::SEED_MINT;

// Two token-program fields with classic Token declared first: the mint's
// `mint::token_program = token_2022_program` must route its `init` CPI to
// Token-2022, not to the first token-program field.
#[derive(Accounts)]
#[instruction(id: u64, mint_bump: u8, decimals: u8)]
pub struct CreateMint2022DualTokenProgram {
    #[account(mut)]
    pub payer: Signer,

    /// SAFETY: `init` + `mint::decimals`/`mint::authority` below fully
    /// validate and construct this account via a real CPI — the target
    /// program is `token_2022_program`, selected explicitly via
    /// `mint::token_program`, not by field order.
    #[account(
        init,
        payer = payer,
        seeds = [SEED_MINT, &id.to_le_bytes()],
        bump = mint_bump,
        mint::decimals = decimals,
        mint::authority = mint_authority,
        mint::token_program = token_2022_program,
    )]
    pub mint: AccountInfo,

    pub mint_authority: Account<MintAuthority>,

    pub token_program: Program<Token>,
    pub token_2022_program: Program<Token2022>,
    pub system_program: Program<System>,
}

pub fn create_mint2022_dual_token_program(
    _ctx: Context<CreateMint2022DualTokenProgram>,
    _id: u64,
    _mint_bump: u8,
    _decimals: u8,
) -> Result {
    Ok(())
}
