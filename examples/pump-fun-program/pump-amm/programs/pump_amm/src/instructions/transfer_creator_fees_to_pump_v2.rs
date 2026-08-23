use naclac_lang::prelude::*;
use crate::constants::{AMM_CREATOR_VAULT_AUTHORITY_SEED, PUMP_CREATOR_VAULT_SEED, PUMP_PROGRAM_ID, WSOL_MINT};
use crate::errors::PumpAmmError;

#[derive(Accounts)]
#[instruction(coin_creator_vault_authority_bump: u8, pump_creator_vault_bump: u8)]
pub struct TransferCreatorFeesToPumpV2 {
    #[account(mut)]
    pub payer: Signer,

    /// SAFETY: only checked against `WSOL_MINT` below; the non-native path
    /// isn't implemented in this scoped pass (see `PumpAmmError::UnsupportedQuoteMint`).
    pub quote_mint: AccountInfo,

    pub token_program: Program<Token>,
    pub system_program: Program<System>,
    pub associated_token_program: Program<AssociatedToken>,

    /// SAFETY: only used as seed material for `coin_creator_vault_authority`/
    /// `pump_creator_vault` below — `pump_fees` passes its own `sharing_config`
    /// PDA here, never dereferenced.
    pub coin_creator: AccountInfo,

    /// SAFETY: the `seeds`/`bump` constraint already verifies its address;
    /// it's a lamport-only PDA authority (no stored data), used below as a
    /// CPI signer and lamport destination/source, never deserialized.
    #[account(
        mut,
        seeds = [AMM_CREATOR_VAULT_AUTHORITY_SEED, coin_creator.address().as_ref()],
        bump = coin_creator_vault_authority_bump,
    )]
    pub coin_creator_vault_authority: AccountInfo,

    #[account(mut)]
    pub coin_creator_vault_ata: Account<TokenAccount>,

    /// SAFETY: the `seeds`/`bump`/`seeds::program` constraint already verifies
    /// its address; it's a lamport-only PDA under `pump`'s own program (no
    /// stored data from this program's perspective), only ever a lamport
    /// destination below, never deserialized.
    #[account(
        mut,
        seeds = [PUMP_CREATOR_VAULT_SEED, coin_creator.address().as_ref()],
        seeds::program = PUMP_PROGRAM_ID,
        bump = pump_creator_vault_bump,
    )]
    pub pump_creator_vault: AccountInfo,

    /// SAFETY: only touched by the non-native-quote path, which this scoped
    /// pass doesn't implement.
    #[account(mut)]
    pub pump_creator_vault_ata: AccountInfo,
}

/// Real `transfer_creator_fees_to_pump_v2` supports arbitrary `quote_mint`s;
/// this scoped pass only implements the wrapped-SOL path (matches
/// `bonding-curve-01-overview.md`'s SOL-quote-only scope decision), and
/// reverts `UnsupportedQuoteMint` for anything else. The WSOL path's
/// mechanics are identical to `transfer_creator_fees_to_pump` (v1) — see
/// that instruction's doc comment.
#[instruction]
pub fn transfer_creator_fees_to_pump_v2(
    ctx: Context<TransferCreatorFeesToPumpV2>,
    coin_creator_vault_authority_bump: u8,
    _pump_creator_vault_bump: u8,
) -> Result {
    require!(
        ctx.accounts.quote_mint.address() == WSOL_MINT,
        PumpAmmError::UnsupportedQuoteMint
    );

    const SPL_TOKEN_ACCOUNT_LEN: usize = 165;
    let token_account_rent_exempt_minimum = Rent::get()?.try_minimum_balance(SPL_TOKEN_ACCOUNT_LEN)?;
    let vault_lamports_before = ctx.accounts.coin_creator_vault_ata.to_account_info().lamports();
    if vault_lamports_before < token_account_rent_exempt_minimum {
        return Ok(());
    }

    let coin_creator_address = ctx.accounts.coin_creator.address();
    let signer_seeds: &[&[u8]] = &[
        AMM_CREATOR_VAULT_AUTHORITY_SEED,
        coin_creator_address.as_ref(),
        &[coin_creator_vault_authority_bump],
    ];
    let signer: &[&[&[u8]]] = &[signer_seeds];

    let authority_baseline = ctx.accounts.coin_creator_vault_authority.lamports();
    let authority_for_signing = ctx.accounts.coin_creator_vault_authority;

    ctx.accounts.token_program.close_account_signed(
        CloseAccountAccounts {
            account: &mut ctx.accounts.coin_creator_vault_ata,
            destination: &mut ctx.accounts.coin_creator_vault_authority,
            authority: &authority_for_signing,
        },
        signer,
    )?;

    ctx.accounts.associated_token_program.create_signed(
        CreateAtaAccounts {
            payer: &mut ctx.accounts.coin_creator_vault_authority,
            associated_token: &mut ctx.accounts.coin_creator_vault_ata,
            authority: &authority_for_signing,
            mint: &ctx.accounts.quote_mint,
            system_program: &ctx.accounts.system_program,
            token_program: &ctx.accounts.token_program,
        },
        signer,
    )?;

    let authority_after_recreate = ctx.accounts.coin_creator_vault_authority.lamports();
    let to_forward = authority_after_recreate.saturating_sub(authority_baseline);
    
    if to_forward > 0 {
        ctx.accounts.system_program.transfer_signed(
            SystemTransferAccounts {
                from: &mut ctx.accounts.coin_creator_vault_authority,
                to: &mut ctx.accounts.pump_creator_vault,
            },
            to_forward,
            signer,
        )?;
    }

    Ok(())
}
