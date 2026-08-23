use naclac_lang::prelude::*;
use crate::components::MintAuthority;
use crate::constants::SEED_MINT;

/// Real end-to-end proof that `init_if_needed` re-validates `mint::decimals`/
/// `mint::authority` against an already-existing mint, not just a
/// freshly-created one (`naclac-macros/src/instruction/security.rs`'s Token/
/// Mint Validation block, gated on `field.init_config.is_none() ||
/// field.is_init_if_needed`). Otherwise identical to `create_mint2022.rs`.
///
/// `mint` is declared *before* `mint_authority` deliberately: `mint::authority
/// = mint_authority` is resolved by `security.rs`'s `resolve_smart_key` via
/// indexed `accounts[idx]` access into the raw pre-walked slice, not a typed
/// local variable — so the referenced field may be declared anywhere in the
/// struct relative to the field whose constraint references it. This
/// ordering was previously required (a real compile error otherwise); see
/// `naclac-macros/docs/derive-accounts-gaps-audit.md`'s gap #10.
#[derive(Accounts)]
#[instruction(id: u64, mint_bump: u8, decimals: u8)]
pub struct CreateMint2022InitIfNeeded {
    #[account(mut)]
    pub payer: Signer,

    /// SAFETY: `init_if_needed` + `mint::decimals`/`mint::authority` below
    /// fully validate and construct this account via a real CPI to the
    /// Token-2022 program (`init_cpi.rs`) — there is no naclac
    /// `Discriminator` to check since this is a raw SPL `Mint` layout.
    #[account(
        init_if_needed,
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

#[instruction]
pub fn create_mint2022_init_if_needed(
    _ctx: Context<CreateMint2022InitIfNeeded>,
    _id: u64,
    _mint_bump: u8,
    _decimals: u8,
) -> Result {
    Ok(())
}
