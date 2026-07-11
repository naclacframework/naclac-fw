use naclac_lang::prelude::*;

declare_id!("4brUZ7qbbQvHFLoVZuUdVejWXUx8WWZpZEgbBs5W49xm");

pub mod components;
pub mod constants;
pub mod instructions;

use instructions::*;

#[program]
pub mod accounts_constraints_zc {
    pub fn init_vault(ctx: Context<InitVault>) -> Result {
        init_vault::init_vault(ctx)
    }

    pub fn init_if_needed_ledger(ctx: Context<InitIfNeededLedger>, value: u64) -> Result {
        init_if_needed_ledger::init_if_needed_ledger(ctx, value)
    }

    pub fn require_signer(ctx: Context<RequireSigner>) -> Result {
        require_signer::require_signer(ctx)
    }

    pub fn check_owner(ctx: Context<CheckOwner>) -> Result {
        check_owner::check_owner(ctx)
    }

    pub fn check_address(ctx: Context<CheckAddress>) -> Result {
        check_address::check_address(ctx)
    }

    pub fn touch_mut_vault(ctx: Context<TouchMutVault>) -> Result {
        touch_mut_vault::touch_mut_vault(ctx)
    }

    pub fn related_vault(ctx: Context<RelatedVault>) -> Result {
        related_vault::related_vault(ctx)
    }

    pub fn init_seeded(ctx: Context<InitSeeded>) -> Result {
        init_seeded::init_seeded(ctx)
    }

    pub fn touch_seeded(ctx: Context<TouchSeeded>, bump: u8) -> Result {
        touch_seeded::touch_seeded(ctx, bump)
    }

    pub fn close_vault(ctx: Context<CloseVault>) -> Result {
        close_vault::close_vault(ctx)
    }
}
