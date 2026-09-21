use naclac_lang::prelude::*;
use crate::components::MintAuthority;
use crate::constants::SEED_MINT;

// Identical to `create_mint.rs` but `token_program` is `Program<Token2022>`
// instead of `Program<Token>` — the first instruction in this test case to
// ever construct a mint against the real Token-2022 program. The
// `mint::decimals`/`mint::authority` codegen (`init_cpi.rs`) already
// dispatches purely on the *runtime* address of whatever account is passed
// as `token_program` (accepting either `TOKEN_PROGRAM_ID` or
// `TOKEN_2022_PROGRAM_ID`, confirmed by reading `init_cpi.rs` directly), so
// this only needed a different static field type to let the *load-time*
// `Program<T>::try_from` check accept the real Token-2022 program address
// (`Token2022::id()` resolves to `TOKEN_2022_PROGRAM_ID`,
// `naclac-core/src/wrappers/program.rs`).
#[derive(Accounts)]
#[instruction(id: u64, mint_bump: u8, decimals: u8)]
pub struct CreateMint2022 {
    #[account(mut)]
    pub payer: Signer,

    /// SAFETY: `init` + `mint::decimals`/`mint::authority` below fully
    /// validate and construct this account via a real CPI to the Token-2022
    /// program (`init_cpi.rs`) — there is no naclac `Discriminator` to check
    /// since this is a raw SPL `Mint` layout, so `AccountInfo` is correct
    /// here, not a gap in coverage.
    #[account(
        init,
        payer = payer,
        seeds = [SEED_MINT, &id.to_le_bytes()],
        bump = mint_bump,
        mint::decimals = decimals,
        mint::authority = mint_authority,
    )]
    pub mint: AccountInfo,

    pub mint_authority: Account<MintAuthority>,

    pub token_program: Program<Token2022>,
    pub system_program: Program<System>,
}

pub fn create_mint2022(
    _ctx: Context<CreateMint2022>,
    _id: u64,
    _mint_bump: u8,
    _decimals: u8,
) -> Result {
    Ok(())
}
