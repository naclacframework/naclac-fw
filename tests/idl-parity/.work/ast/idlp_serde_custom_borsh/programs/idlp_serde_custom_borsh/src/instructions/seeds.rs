//! PDA seed expressions in every shape the IDL has to classify. The first struct needs no
//! instruction arguments; the second takes them. Two literal seeds contain the substrings
//! `mut` and `init` without being those keywords.

use crate::components::{Entry, Ledger, Registry};
use crate::constants::{OTHER_PROGRAM, SEED_ENTRY, SEED_LEDGER, SEED_REGISTRY, SEED_TEXT, TREASURY};
use naclac_lang::prelude::*;

#[derive(Accounts)]
pub struct SeedFormsLiteral {
    #[account(seeds = [b"lit"], bump)]
    pub by_byte_string: Account<Entry>,
    #[account(seeds = [b"mutable_seed"], bump)]
    pub by_mut_text: Account<Entry>,
    #[account(seeds = [b"initial_seed"], bump)]
    pub by_init_text: Account<Entry>,
    #[account(seeds = [SEED_REGISTRY], bump)]
    pub by_const_bytes: Account<Registry>,
    #[account(seeds = [SEED_LEDGER], bump)]
    pub by_const_array: Account<Ledger>,
    #[account(seeds = [SEED_TEXT.as_bytes()], bump)]
    pub by_const_str: Account<Entry>,
    #[account(seeds = [b"int", &[5u8]], bump)]
    pub by_int_byte: Account<Entry>,
    #[account(
        seeds = [b"le", &1u16.to_le_bytes(), &70000u32.to_le_bytes(), &9u64.to_le_bytes(), &11u128.to_le_bytes()],
        bump
    )]
    pub by_int_le: Account<Entry>,
    #[account(seeds = [b"be", &2u32.to_be_bytes(), &3u64.to_be_bytes()], bump)]
    pub by_int_be: Account<Entry>,
}

pub fn seed_forms_literal(_ctx: Context<SeedFormsLiteral>) -> Result {
    Ok(())
}

#[derive(Accounts)]
#[instruction(id: u64, platform: u8, label: [u8; 8], bump: u8)]
pub struct SeedFormsDynamic {
    pub payer: Signer,
    pub registry: Account<Registry>,
    /// SAFETY: used only as a seed source.
    pub config_account: AccountInfo,
    #[account(seeds = [b"acct", registry.as_ref()], bump)]
    pub by_account: Account<Entry>,
    #[account(seeds = [b"addr", payer.address().as_ref()], bump)]
    pub by_account_address: Account<Entry>,
    #[account(seeds = [b"field", registry.label.as_ref()], bump)]
    pub by_account_field: Account<Entry>,
    #[account(
        seeds = [b"fle", registry.bump.to_le_bytes().as_ref(), registry.count.to_le_bytes().as_ref()],
        bump
    )]
    pub by_field_le: Account<Entry>,
    #[account(seeds = [b"cfg", config_account.address().as_ref()], bump)]
    pub by_config_account: Account<Entry>,
    #[account(seeds = [b"arg", id.to_le_bytes().as_ref()], bump)]
    pub by_arg_le: Account<Entry>,
    #[account(seeds = [b"argref", &id.to_le_bytes()], bump)]
    pub by_arg_ref_le: Account<Entry>,
    #[account(seeds = [b"plat", &[platform]], bump)]
    pub by_arg_byte: Account<Entry>,
    #[account(seeds = [b"lbl", label.as_ref()], bump)]
    pub by_arg_array: Account<Entry>,
    #[account(
        seeds = [SEED_ENTRY, b"x", registry.as_ref(), payer.address().as_ref(), &id.to_le_bytes(), &[platform]],
        bump
    )]
    pub by_many: Account<Entry>,
    #[account(seeds = [b"expl"], bump = bump)]
    pub by_explicit_bump: Account<Entry>,
    /// SAFETY: only its address is checked against a PDA of another program.
    #[account(seeds = [b"prog"], bump = bump, seeds::program = TREASURY)]
    pub by_program_tagged_const: AccountInfo,
    /// SAFETY: only its address is checked against a PDA of another program.
    #[account(seeds = [b"prog2"], bump = bump, seeds::program = OTHER_PROGRAM)]
    pub by_program_untagged_const: AccountInfo,
    /// SAFETY: only its address is checked against a PDA of another program.
    #[account(seeds = [b"prog3"], bump = bump, seeds::program = naclac_lang::prelude::TOKEN_PROGRAM_ID)]
    pub by_program_path: AccountInfo,
    /// SAFETY: only its address is checked against a PDA of another program.
    #[account(
        seeds = [b"prog4"],
        bump = bump,
        seeds::program = address!("ATokenGPvbdGVxr1b2hvZbsiqW5xWH25efTNsLJA8knL")
    )]
    pub by_program_macro: AccountInfo,
}

pub fn seed_forms_dynamic(
    _ctx: Context<SeedFormsDynamic>,
    _id: u64,
    _platform: u8,
    _label: [u8; 8],
    _bump: u8,
) -> Result {
    Ok(())
}
