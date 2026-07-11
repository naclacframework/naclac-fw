#![no_std]
use naclac_lang::prelude::*;

declare_id!("G2V85CrtmdvgCCTr1e38gSWhw4Leo5p9EYM89bFNrPAa");

pub mod components;
pub mod constants;
pub mod instructions;

use instructions::*;

#[program]
pub mod pda_seeds {
    pub fn init_registry(ctx: Context<InitRegistry>) -> Result {
        init_registry::init_registry(ctx)
    }

    pub fn init_entry(ctx: Context<InitEntry>, bump: u8) -> Result {
        init_entry::init_entry(ctx, bump)
    }

    pub fn touch_entry_bare_bump(ctx: Context<TouchEntryBareBump>) -> Result {
        touch_entry_bare_bump::touch_entry_bare_bump(ctx)
    }

    pub fn touch_registry_explicit_bump(
        ctx: Context<TouchRegistryExplicitBump>,
        bump: u8,
    ) -> Result {
        touch_registry_explicit_bump::touch_registry_explicit_bump(ctx, bump)
    }

    pub fn init_child(ctx: Context<InitChild>, bump: u8) -> Result {
        init_child::init_child(ctx, bump)
    }

    pub fn init_tagged_child(ctx: Context<InitTaggedChild>, bump: u8) -> Result {
        init_tagged_child::init_tagged_child(ctx, bump)
    }
}
