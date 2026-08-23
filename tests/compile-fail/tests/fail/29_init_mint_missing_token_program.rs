#![allow(unexpected_cfgs)]
// Site: naclac-macros/src/instruction/init_cpi.rs's
// `resolve_token_program_idx` — an `init` `mint::decimals =`/`mint::authority
// =` field with no `Program<Token>`/`Program<Token2022>`/
// `Interface<TokenInterface>` anywhere in the struct. Previously a raw
// proc-macro panic; now a spanned `syn::Error`.
use naclac_lang::prelude::*;

#[derive(Accounts)]
pub struct Foo {
    #[account(mut)]
    pub payer: Signer,

    /// SAFETY: only used as the mint's `authority` for constraint
    /// verification below; never read or deserialized.
    pub authority: AccountInfo,

    /// SAFETY: fully validated/constructed via `mint::decimals =`/`init`
    /// below — irrelevant here, this fixture never reaches that check.
    #[account(
        init,
        payer = payer,
        seeds = [b"mint"],
        bump = 255,
        mint::decimals = 6,
        mint::authority = authority,
    )]
    pub mint: AccountInfo,

    pub system_program: Program<System>,
}

fn main() {}
