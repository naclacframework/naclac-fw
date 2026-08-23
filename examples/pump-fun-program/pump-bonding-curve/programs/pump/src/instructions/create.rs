use naclac_lang::prelude::*;
use crate::components::{BondingCurve, Global};
use crate::constants::{
    BONDING_CURVE_SEED, GLOBAL_SEED, METADATA_SEED, MINT_AUTHORITY_SEED, MINT_DECIMALS,
    MPL_TOKEN_METADATA_PROGRAM_ID,
};
use crate::events::CreateEvent;
use crate::systems::{create_metadata_via_cpi, CreateMetadataCpiAccounts};

#[derive(Accounts)]
#[instruction(name: ZcString, symbol: ZcString, uri: ZcString, creator: Address, bonding_curve_bump: u8, metadata_bump: u8)]
pub struct Create {
    #[account(mut)]
    pub user: Signer,
    
    /// SAFETY: the `seeds`/`bump` constraint fully validates this; it's a
    /// lamport-only PDA (no stored data) — only ever used as the mint's
    /// authority, signed via its own seeds for the `mint_to` CPI below.
    #[account(seeds = [MINT_AUTHORITY_SEED], bump)]
    pub mint_authority: AccountInfo,

    #[account(init, payer = user, mint::decimals = MINT_DECIMALS, mint::authority = mint_authority)]
    pub mint: Signer,

    #[account(
        init,
        payer = user,
        seeds = [BONDING_CURVE_SEED, mint.address().as_ref()],
        bump = bonding_curve_bump,
    )]
    pub bonding_curve: Account<BondingCurve>,

    #[account(
        init,
        payer = user,
        associated_token::mint = mint,
        associated_token::authority = bonding_curve,
    )]
    pub associated_bonding_curve: Account<TokenAccount>,

    #[account(seeds = [GLOBAL_SEED], bump)]
    pub global: Account<Global>,

    /// SAFETY: the `address` constraint fully validates this; only used as
    /// the target program of the `create_metadata_via_cpi` call below, never
    /// deserialized.
    #[account(address = MPL_TOKEN_METADATA_PROGRAM_ID)]
    pub mpl_token_metadata: AccountInfo,

    /// SAFETY: the `seeds`/`bump`/`seeds::program` constraint fully verifies
    /// this is the real Metaplex metadata PDA for `mint`; never deserialized
    /// here — `mpl_token_metadata`'s own `CreateMetadataAccountV3` handler
    /// initializes it during the CPI below.
    #[account(
        mut,
        seeds = [METADATA_SEED, MPL_TOKEN_METADATA_PROGRAM_ID.as_ref(), mint.address().as_ref()],
        seeds::program = MPL_TOKEN_METADATA_PROGRAM_ID,
        bump = metadata_bump,
    )]
    pub metadata: AccountInfo,

    pub system_program: Program<System>,
    pub token_program: Program<Token>,
    pub associated_token_program: Program<AssociatedToken>,
}

#[instruction]
pub fn create(
    ctx: Context<Create>,
    name: ZcString,
    symbol: ZcString,
    uri: ZcString,
    creator: Address,
    _bonding_curve_bump: u8,
    _metadata_bump: u8,
) -> Result {
    let global = &ctx.accounts.global;
    let token_total_supply = global.token_total_supply;
    let virtual_token_reserves = global.initial_virtual_token_reserves;
    let virtual_sol_reserves = global.initial_virtual_sol_reserves;
    let real_token_reserves = global.initial_real_token_reserves;

    let bonding_curve = &mut ctx.accounts.bonding_curve;
    bonding_curve.creator = creator;
    bonding_curve.virtual_token_reserves = virtual_token_reserves;
    bonding_curve.virtual_quote_reserves = virtual_sol_reserves;
    bonding_curve.real_token_reserves = real_token_reserves;
    bonding_curve.token_total_supply = token_total_supply;

    let mint_authority_signer_seeds: &[&[u8]] = &[MINT_AUTHORITY_SEED, &[ctx.bumps.mint_authority]];
    let mint_authority_signer: &[&[&[u8]]] = &[mint_authority_signer_seeds];
    ctx.accounts.token_program.mint_to_signed(
        MintToAccounts {
            mint: &mut ctx.accounts.mint,
            to: &mut ctx.accounts.associated_bonding_curve,
            authority: &ctx.accounts.mint_authority,
        },
        token_total_supply,
        mint_authority_signer,
    )?;

    create_metadata_via_cpi(
        CreateMetadataCpiAccounts {
            metadata: ctx.accounts.metadata.view,
            mint: ctx.accounts.mint.view,
            mint_authority: ctx.accounts.mint_authority.view,
            payer: ctx.accounts.user.view,
            system_program: ctx.accounts.system_program.view,
        },
        name.as_str(),
        symbol.as_str(),
        uri.as_str(),
        ctx.bumps.mint_authority,
    )?;

    let timestamp = unix_timestamp()?;
    emit!(CreateEvent {
        name: String::from(name.as_str()),
        symbol: String::from(symbol.as_str()),
        uri: String::from(uri.as_str()),
        mint: ctx.accounts.mint.address(),
        bonding_curve: ctx.accounts.bonding_curve.address(),
        user: ctx.accounts.user.address(),
        creator,
        timestamp,
        virtual_token_reserves,
        virtual_sol_reserves,
        real_token_reserves,
        token_total_supply,
        token_program: ctx.accounts.token_program.address(),
        is_mayhem_mode: Bool::from(false),
        is_cashback_enabled: Bool::from(false),
        quote_mint: Address::default(),
        virtual_quote_reserves: virtual_sol_reserves,
    });

    msg!("Coin successfully created");
    Ok(())
}
