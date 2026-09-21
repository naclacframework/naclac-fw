use naclac_lang::prelude::*;

declare_id!("8wrsJkY2VBYUC33JTMHUeJLZfjXypcCC8dos5pEZJpYa");

pub mod components;
pub mod constants;
pub mod instructions;

use instructions::*;

#[program]
pub mod token_zc {
    pub fn init_mint_authority(ctx: Context<InitMintAuthority>) -> Result {
        init_mint_authority::init_mint_authority(ctx)
    }

    pub fn create_mint(
        ctx: Context<CreateMint>,
        id: u64,
        mint_bump: u8,
        decimals: u8,
    ) -> Result {
        create_mint::create_mint(ctx, id, mint_bump, decimals)
    }

    pub fn create_mint_with_freeze(
        ctx: Context<CreateMintWithFreeze>,
        id: u64,
        mint_bump: u8,
        decimals: u8,
    ) -> Result {
        create_mint_with_freeze::create_mint_with_freeze(ctx, id, mint_bump, decimals)
    }

    pub fn check_vault_constraints(ctx: Context<CheckVaultConstraints>) -> Result {
        check_vault_constraints::check_vault_constraints(ctx)
    }

    pub fn check_vault_program(ctx: Context<CheckVaultProgram>) -> Result {
        check_vault_program::check_vault_program(ctx)
    }

    pub fn check_mint_freeze_authority(ctx: Context<CheckMintFreezeAuthority>) -> Result {
        check_mint_freeze_authority::check_mint_freeze_authority(ctx)
    }

    pub fn mint_to_vault(ctx: Context<MintToVault>, amount: u64) -> Result {
        mint_to_vault::mint_to_vault(ctx, amount)
    }

    pub fn transfer_tokens(ctx: Context<TransferTokens>, amount: u64) -> Result {
        transfer_tokens::transfer_tokens(ctx, amount)
    }

    pub fn create_ata(ctx: Context<CreateAssociatedTokenAccount>) -> Result {
        create_ata::create_ata(ctx)
    }

    pub fn create_ata_idempotent(ctx: Context<CreateAssociatedTokenAccountIdempotent>) -> Result {
        create_ata_idempotent::create_ata_idempotent(ctx)
    }

    pub fn check_ata_constraints(ctx: Context<CheckAtaConstraints>, ata_bump: u8) -> Result {
        check_ata_constraints::check_ata_constraints(ctx, ata_bump)
    }

    pub fn burn_vault_tokens(ctx: Context<BurnVaultTokens>, amount: u64) -> Result {
        burn_vault_tokens::burn_vault_tokens(ctx, amount)
    }

    pub fn set_mint_authority(ctx: Context<SetMintAuthority>, authority_type: u8) -> Result {
        set_mint_authority::set_mint_authority(ctx, authority_type)
    }

    pub fn freeze_vault_account(ctx: Context<FreezeVaultAccount>) -> Result {
        freeze_vault_account::freeze_vault_account(ctx)
    }

    pub fn thaw_vault_account(ctx: Context<ThawVaultAccount>) -> Result {
        thaw_vault_account::thaw_vault_account(ctx)
    }

    pub fn approve_vault_delegate(ctx: Context<ApproveVaultDelegate>, amount: u64) -> Result {
        approve_vault_delegate::approve_vault_delegate(ctx, amount)
    }

    pub fn revoke_vault_delegate(ctx: Context<RevokeVaultDelegate>) -> Result {
        revoke_vault_delegate::revoke_vault_delegate(ctx)
    }

    pub fn close_vault_account(ctx: Context<CloseVaultAccount>) -> Result {
        close_vault_account::close_vault_account(ctx)
    }

    pub fn transfer_tokens_checked(
        ctx: Context<TransferTokensChecked>,
        amount: u64,
        decimals: u8,
    ) -> Result {
        transfer_tokens_checked::transfer_tokens_checked(ctx, amount, decimals)
    }

    pub fn create_mint2022(
        ctx: Context<CreateMint2022>,
        id: u64,
        mint_bump: u8,
        decimals: u8,
    ) -> Result {
        create_mint2022::create_mint2022(ctx, id, mint_bump, decimals)
    }

    pub fn mint_to_vault2022(ctx: Context<MintToVault2022>, amount: u64) -> Result {
        mint_to_vault2022::mint_to_vault2022(ctx, amount)
    }

    pub fn transfer_tokens2022(ctx: Context<TransferTokens2022>, amount: u64) -> Result {
        transfer_tokens2022::transfer_tokens2022(ctx, amount)
    }
}
