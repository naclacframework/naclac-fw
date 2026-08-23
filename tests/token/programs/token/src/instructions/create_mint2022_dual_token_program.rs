use naclac_lang::prelude::*;
use crate::components::MintAuthority;
use crate::constants::SEED_MINT;

// Regression fixture for the `init_cpi.rs` bug where `init`'s CPI-target
// scan always picked the *first* `Program<Token>`/`Program<Token2022>`/
// `Interface<TokenInterface>` field in the struct, ignoring any explicit
// `token::program = X` pointer. `token_program` (classic Token) is
// deliberately declared *before* `token_2022_program`, and the mint is
// explicitly routed to the second field via `token::program =
// token_2022_program` — before the fix this would have silently created
// the mint under classic Token instead.
#[derive(Accounts)]
#[instruction(id: u64, mint_bump: u8, decimals: u8)]
pub struct CreateMint2022DualTokenProgram {
    #[account(mut)]
    pub payer: Signer,

    /// SAFETY: `init` + `mint::decimals`/`mint::authority` below fully
    /// validate and construct this account via a real CPI — the target
    /// program is `token_2022_program`, selected explicitly via
    /// `token::program`, not by field order.
    #[account(
        init,
        payer = payer,
        seeds = [SEED_MINT, &id.to_le_bytes()],
        bump = mint_bump,
        mint::decimals = decimals,
        mint::authority = mint_authority,
        token::program = token_2022_program,
    )]
    pub mint: AccountInfo,

    pub mint_authority: Account<MintAuthority>,

    pub token_program: Program<Token>,
    pub token_2022_program: Program<Token2022>,
    pub system_program: Program<System>,
}

#[instruction]
pub fn create_mint2022_dual_token_program(
    _ctx: Context<CreateMint2022DualTokenProgram>,
    _id: u64,
    _mint_bump: u8,
    _decimals: u8,
) -> Result {
    Ok(())
}
