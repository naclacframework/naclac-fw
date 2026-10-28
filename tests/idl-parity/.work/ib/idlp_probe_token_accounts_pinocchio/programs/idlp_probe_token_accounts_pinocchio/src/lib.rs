//! Probe: typed token accounts (`Account<Mint>`, `Account<TokenAccount>`, `InterfaceAccount<..>`) and the
//! constraints that need them, under the compiled `idl-build` path.

use naclac_lang::prelude::*;

declare_id!("BKwRZDJUJA9Yb5s1EHPrK6YQhGg6V1FwHaLFTrGspmBN");

#[derive(Accounts)]
#[instruction(ata_bump: u8)]
pub struct TokenAccounts {
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

fn token_accounts_impl(_ctx: Context<TokenAccounts>, _ata_bump: u8) -> Result {
    Ok(())
}

#[program]
pub mod idlp_probe_token_accounts_pinocchio {
    pub fn token_accounts(ctx: Context<TokenAccounts>, ata_bump: u8) -> Result {
        token_accounts_impl(ctx, ata_bump)
    }
}
