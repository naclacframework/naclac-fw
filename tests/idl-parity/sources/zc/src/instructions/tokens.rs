//! Token, mint and associated-token constraints: creation (`init`, `init_if_needed`) and checks.

use crate::components::Roles;
use crate::constants::SEED_ENTRY;
use naclac_lang::prelude::*;

#[derive(Accounts)]
#[instruction(id: u64, mint_bump: u8, decimals: u8)]
pub struct CreateMint {
    #[account(mut)]
    pub payer: Signer,
    /// SAFETY: created by this instruction and initialized as a mint.
    #[account(
        init,
        payer = payer,
        seeds = [SEED_ENTRY, &id.to_le_bytes()],
        bump = mint_bump,
        mint::decimals = decimals,
        mint::authority = mint_authority,
        mint::freeze_authority = mint_authority,
    )]
    pub mint: AccountInfo,
    pub mint_authority: Account<Roles>,
    pub token_program: Program<Token>,
    pub system_program: Program<System>,
}

pub fn create_mint(_ctx: Context<CreateMint>, _id: u64, _mint_bump: u8, _decimals: u8) -> Result {
    Ok(())
}

#[derive(Accounts)]
pub struct CreateMint2022 {
    #[account(mut)]
    pub payer: Signer,
    /// SAFETY: created by this instruction and initialized as a mint.
    #[account(
        init,
        payer = payer,
        seeds = [b"mint22"],
        bump,
        mint::decimals = 6,
        mint::authority = mint_authority,
    )]
    pub mint: AccountInfo,
    pub mint_authority: Account<Roles>,
    pub token_program: Program<Token2022>,
    pub system_program: Program<System>,
}

pub fn create_mint_2022(_ctx: Context<CreateMint2022>) -> Result {
    Ok(())
}

#[derive(Accounts)]
pub struct CreateAta {
    #[account(mut)]
    pub payer: Signer,
    pub owner: Signer,
    /// SAFETY: only its address is used as the token mint.
    pub mint: AccountInfo,
    /// SAFETY: created by this instruction as the owner's associated token account.
    #[account(
        init,
        payer = payer,
        associated_token::mint = mint,
        associated_token::authority = owner,
    )]
    pub ata: AccountInfo,
    /// SAFETY: created when missing, as the owner's associated token account.
    #[account(
        mut,
        init_if_needed,
        payer = payer,
        associated_token::mint = mint,
        associated_token::authority = owner,
    )]
    pub ata_idempotent: AccountInfo,
    pub system_program: Program<System>,
    pub token_program: Program<Token>,
    pub associated_token_program: Program<AssociatedToken>,
}

pub fn create_ata(_ctx: Context<CreateAta>) -> Result {
    Ok(())
}

#[derive(Accounts)]
pub struct CheckTokenProgram {
    pub token_program: Program<Token>,
    /// SAFETY: only its token program is checked.
    #[account(token::token_program = token_program)]
    pub program_checked: AccountInfo,
}

pub fn check_token_program(_ctx: Context<CheckTokenProgram>) -> Result {
    msg!("check_token_program");
    Ok(())
}

#[derive(Accounts)]
#[instruction(ata_bump: u8)]
pub struct TypedTokenAccounts {
    pub mint: Account<Mint>,
    pub owner: Signer,
    #[account(
        associated_token::mint = mint,
        associated_token::authority = owner,
        associated_token::bump = ata_bump,
    )]
    pub ata: Account<TokenAccount>,
    #[account(mut, token::mint = mint, token::authority = owner)]
    pub vault: Account<TokenAccount>,
    pub token_program: Program<Token>,
    pub freeze_authority: Signer,
    #[account(mint::freeze_authority = freeze_authority)]
    pub frozen_mint: Account<Mint>,
    pub interface_mint: InterfaceAccount<Mint>,
    pub interface_token: InterfaceAccount<TokenAccount>,
}

pub fn typed_token_accounts(_ctx: Context<TypedTokenAccounts>, _ata_bump: u8) -> Result {
    Ok(())
}
