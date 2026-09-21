#![no_std]
use naclac_lang::prelude::*;

declare_id!("4tsvZRetXKqS2H7kMUGTaArFAjUhmt7W8TF8dwFQ7aiL");

pub mod components;
pub mod constants;
pub mod errors;
pub mod instructions;

use instructions::*;

#[program]
pub mod accounts_constraints {
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

    pub fn check_owner_relational(ctx: Context<CheckOwnerRelational>) -> Result {
        check_owner_relational::check_owner_relational(ctx)
    }

    pub fn check_address(ctx: Context<CheckAddress>) -> Result {
        check_address::check_address(ctx)
    }

    pub fn check_address_relational(ctx: Context<CheckAddressRelational>) -> Result {
        check_address_relational::check_address_relational(ctx)
    }

    pub fn check_executable(ctx: Context<CheckExecutable>) -> Result {
        check_executable::check_executable(ctx)
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

    pub fn close_vault_self(ctx: Context<CloseVaultSelf>) -> Result {
        close_vault_self::close_vault_self(ctx)
    }

    pub fn check_rent_exempt(ctx: Context<CheckRentExempt>) -> Result {
        check_rent_exempt::check_rent_exempt(ctx)
    }

    pub fn related_vault_custom_error(ctx: Context<RelatedVaultCustomError>) -> Result {
        related_vault_custom_error::related_vault_custom_error(ctx)
    }

    pub fn check_external_pda(ctx: Context<CheckExternalPda>, bump: u8) -> Result {
        check_external_pda::check_external_pda(ctx, bump)
    }
}
