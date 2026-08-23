#![allow(unexpected_cfgs)]
// Site: naclac-macros/src/instruction/security.rs — an
// `associated_token::mint =`/`associated_token::authority =` pair on an
// *existing* (non-`init`) account with no `associated_token::bump =`. Like
// `seeds = [...]` without `bump` (17_seeds_missing_bump.rs), this is
// deliberate: naclac never runs `find_program_address` on-chain, so the
// ATA's canonical bump must be supplied explicitly to enable hash-and-compare.
use naclac_lang::prelude::*;

#[derive(Accounts)]
pub struct Foo {
    pub mint: Account<Mint>,

    /// SAFETY: only used as the ATA's `authority` for constraint
    /// verification below; never read or deserialized.
    pub owner: AccountInfo,

    #[account(
        associated_token::mint = mint,
        associated_token::authority = owner,
    )]
    pub associated_token: Account<TokenAccount>,

    pub token_program: Program<Token>,
}

fn main() {}
