use naclac_lang::prelude::*;
use crate::components::MintAuthority;
use crate::constants::SEED_MINT;

// `mint::freeze_authority` combined with `init` — mirrors `create_mint.rs`
// exactly, but additionally sets the freeze authority when creating the
// mint via the real CPI to the token program (`init_cpi.rs`'s
// `non_pinocchio_freeze_logic`/`pinocchio_freeze_logic` branches, ~lines
// 185-211 and ~608-621). `create_mint.rs` never exercised this path — it
// only ever set `mint::decimals`/`mint::authority` — so this instruction
// exists specifically to close that gap. Reuses the `SEED_MINT` prefix;
// PDA uniqueness still comes from the dynamic `id` seed component, same as
// `create_mint`.
#[derive(Accounts)]
#[instruction(id: u64, mint_bump: u8, decimals: u8)]
pub struct CreateMintWithFreeze {
    #[account(mut)]
    pub payer: Signer,

    /// SAFETY: `init` + `mint::decimals`/`mint::authority`/`mint::freeze_authority`
    /// below fully validate and construct this account via a real CPI to the
    /// token program (`init_cpi.rs`) — there is no naclac `Discriminator` to
    /// check since this is a raw SPL `Mint` layout, so `AccountInfo` is
    /// correct here, not a gap in coverage.
    #[account(
        init,
        payer = payer,
        seeds = [SEED_MINT, &id.to_le_bytes()],
        bump = mint_bump,
        mint::decimals = decimals,
        mint::authority = mint_authority,
        mint::freeze_authority = mint_authority,
    )]
    pub mint: AccountInfo,

    pub mint_authority: Account<MintAuthority>,

    pub token_program: Program<Token>,
    pub system_program: Program<System>,
}

#[instruction]
pub fn create_mint_with_freeze(
    _ctx: Context<CreateMintWithFreeze>,
    _id: u64,
    _mint_bump: u8,
    _decimals: u8,
) -> Result {
    Ok(())
}
