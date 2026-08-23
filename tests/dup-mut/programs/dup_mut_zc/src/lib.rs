use naclac_lang::prelude::*;

declare_id!("H2NgEykTkDNkVsNgEoXUiSZ2A9UbyRhNbEXZnbrrFXjj");

pub mod components;
pub mod constants;
pub mod instructions;

use instructions::*;

#[program]
pub mod dup_mut {
    pub fn init_vault_a(ctx: Context<InitVaultA>) -> Result {
        init_vault_a::init_vault_a(ctx)
    }

    pub fn init_vault_b(ctx: Context<InitVaultB>) -> Result {
        init_vault_b::init_vault_b(ctx)
    }

    pub fn touch_pair_no_alias(ctx: Context<TouchPairNoAlias>) -> Result {
        touch_pair_no_alias::touch_pair_no_alias(ctx)
    }

    pub fn touch_pair_with_alias(ctx: Context<TouchPairWithAlias>) -> Result {
        touch_pair_with_alias::touch_pair_with_alias(ctx)
    }

    pub fn init_vault_c(ctx: Context<InitVaultC>) -> Result {
        init_vault_c::init_vault_c(ctx)
    }

    pub fn touch_triple_partial_alias(ctx: Context<TouchTriplePartialAlias>) -> Result {
        touch_triple_partial_alias::touch_triple_partial_alias(ctx)
    }

    pub fn write_note(ctx: Context<WriteNote>, text: ZcString) -> Result {
        write_note::write_note(ctx, text)
    }
}
