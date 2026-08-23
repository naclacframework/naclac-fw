use naclac_lang::prelude::*;
use crate::components::MintAuthority;
use crate::constants::SEED_MINT;

// `mint::decimals` + `mint::authority` combined with `init` — the account
// itself is a bare `AccountInfo` (not `Account<Mint>`), since `init` +
// `mint::*` routes through `init_cpi.rs`'s real CPI-based mint creation,
// not naclac's own Discriminator-prefixed layout. Mirrors
// `examples/launchpad/.../create_mint.rs` exactly. The seed includes a
// dynamic `id` component (not a pure literal) specifically so the explicit
// `bump = mint_bump` is allowed — a pure-literal seed + `init` requires
// bare `bump` instead (`security.rs`'s precomputed-PDA restriction).
#[derive(Accounts)]
#[instruction(id: u64, mint_bump: u8, decimals: u8)]
pub struct CreateMint {
    #[account(mut)]
    pub payer: Signer,

    /// SAFETY: `init` + `mint::decimals`/`mint::authority` below fully
    /// validate and construct this account via a real CPI to the token
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

    pub token_program: Program<Token>,
    pub system_program: Program<System>,
}

#[instruction]
pub fn create_mint(_ctx: Context<CreateMint>, _id: u64, _mint_bump: u8, _decimals: u8) -> Result {
    Ok(())
}
