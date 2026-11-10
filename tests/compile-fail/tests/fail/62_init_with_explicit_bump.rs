#![allow(unexpected_cfgs)]
// Site: derive/src/accounts/field/checks/pda.rs — `init` creates the account
// at the canonical bump, so `bump = <expr>` is rejected on a creating field.
use naclac_lang::prelude::*;

declare_id!("11111111111111111111111111111111");

#[derive(Accounts)]
#[instruction(id: u64, bump: u8)]
pub struct Foo {
    #[account(mut)]
    pub payer: Signer,

    #[account(
        init,
        payer = payer,
        seeds = [b"entry", &id.to_le_bytes()],
        bump = bump
    )]
    pub entry: Account<Entry>,

    pub system_program: Program<System>,
}

#[account]
pub struct Entry {
    pub value: u64,
}

fn main() {}
