#![allow(unexpected_cfgs)]
// Site: naclac-macros/src/instruction/init_cpi.rs — an `init`/`init_if_needed`
// associated-token field with `associated_token::bump = ...`. The real
// Associated Token Program re-derives and checks the canonical address
// itself on every `Create`/`CreateIdempotent` CPI, so naclac never reads
// this value for an init'd ATA; it only matters on an existing (non-init)
// associated_token field.
use naclac_lang::prelude::*;

#[instruction_args]
pub struct FooArgs {
    pub vault_bump: u8,
}

#[derive(Accounts)]
#[instruction(args: FooArgs)]
pub struct Foo {
    #[account(mut)]
    pub payer: Signer,

    pub mint: Account<Mint>,

    /// SAFETY: only used as the ATA's `authority` for constraint
    /// verification below; never read or deserialized.
    pub owner: AccountInfo,

    #[account(
        init,
        payer = payer,
        associated_token::mint = mint,
        associated_token::authority = owner,
        associated_token::bump = args.vault_bump,
    )]
    pub vault: Account<TokenAccount>,

    pub token_program: Program<Token>,
    pub associated_token_program: Program<AssociatedToken>,
    pub system_program: Program<System>,
}

fn main() {}
