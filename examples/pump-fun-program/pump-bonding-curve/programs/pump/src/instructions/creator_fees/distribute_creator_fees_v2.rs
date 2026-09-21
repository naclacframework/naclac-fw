use naclac_lang::prelude::*;
use crate::components::{BondingCurve, Shareholder, SharingConfig};
use crate::constants::{
    BONDING_CURVE_SEED, CREATOR_VAULT_SEED, PUMP_FEES_AUTHORITY_SEED, PUMP_FEES_PROGRAM_ID, SHARING_CONFIG_SEED,
    WSOL_MINT,
};
use crate::errors::PumpError;
use crate::events::DistributeCreatorFeesEvent;

#[derive(Accounts)]
#[instruction(bonding_curve_bump: u8, creator_vault_bump: u8, initialize_ata: Bool)]
pub struct DistributeCreatorFeesV2 {
    #[account(mut)]
    pub payer: Signer,

    /// SAFETY: only used as a seed input for `bonding_curve`/`sharing_config` below, never
    /// read or written — a wrong value just fails those seed checks.
    pub mint: AccountInfo,

    #[account(seeds = [BONDING_CURVE_SEED, mint.address().as_ref()], bump = bonding_curve_bump)]
    pub bonding_curve: Account<BondingCurve>,

    #[account(
        seeds = [SHARING_CONFIG_SEED, mint.address().as_ref()],
        seeds::program = PUMP_FEES_PROGRAM_ID,
        bump = sharing_config.bump,
        owner = PUMP_FEES_PROGRAM_ID,
    )]
    pub sharing_config: Account<SharingConfig>,

    /// SAFETY: the `seeds`/`bump` constraint already verifies its address;
    /// it's a lamport-only PDA (no stored data), never `init`'d so still
    /// System-owned — only ever a lamport source below via a signed System
    /// Program transfer, never deserialized.
    #[account(
        mut,
        seeds = [CREATOR_VAULT_SEED, bonding_curve.creator.as_ref()],
        bump = creator_vault_bump,
    )]
    pub creator_vault: AccountInfo,

    pub system_program: Program<System>,

    /// SAFETY: only touched by the non-native-quote path, which this scoped
    /// pass doesn't implement — see `UnsupportedQuoteMint` below.
    #[account(mut)]
    pub creator_vault_quote_token_account: AccountInfo,

    /// SAFETY: only used to check `quote_mint.address() == WSOL_MINT` below.
    pub quote_mint: AccountInfo,

    pub quote_token_program: Program<Token>,

    pub associated_token_program: Program<AssociatedToken>,

    /// SAFETY: `signer` + the `seeds`/`seeds::program` constraint together
    /// prove this call was CPI'd (via `invoke_signed`) by `pump_fees` itself
    /// — only that program can ever produce a valid signature for its own
    /// `PUMP_FEES_AUTHORITY_SEED` PDA. This is the entire authorization
    /// model for this instruction; never deserialized.
    #[account(
        signer,
        seeds = [PUMP_FEES_AUTHORITY_SEED],
        seeds::program = PUMP_FEES_PROGRAM_ID,
        bump,
    )]
    pub pump_fees_authority: AccountInfo,
}

/// Real `distribute_creator_fees_v2` supports arbitrary `quote_mint`s; this
/// scoped pass only implements the wrapped-SOL path (matches
/// `bonding-curve-01-overview.md`'s SOL-quote-only scope decision), and
/// reverts `UnsupportedQuoteMint` for anything else. The WSOL path's
/// distribution math and rounding rule are identical to `distribute_creator_fees`
/// (v1) — see that instruction's doc comment.
///
/// Requires `bonding_curve.creator == sharing_config`'s own address (real,
/// live-confirmed check, `reference/fee-tier-probe/src/bin/probe71.rs`) --
/// see `distribute_creator_fees`'s own doc comment for why. Also rejects any
/// executable shareholder recipient (real, live-confirmed check,
/// `reference/fee-tier-probe/src/bin/probe73.rs`) — see that same doc
/// comment for why.
pub fn distribute_creator_fees_v2(
    ctx: Context<DistributeCreatorFeesV2>,
    _bonding_curve_bump: u8,
    creator_vault_bump: u8,
    _initialize_ata: Bool,
) -> Result {
    require!(
        ctx.accounts.quote_mint.address() == WSOL_MINT,
        PumpError::UnsupportedQuoteMint
    );
    require!(
        ctx.accounts.bonding_curve.creator == ctx.accounts.sharing_config.address(),
        PumpError::BondingCurveAndSharingConfigCreatorMismatch
    );

    let shareholders_len = ctx.accounts.sharing_config.shareholders_len as usize;
    require!(shareholders_len > 0, PumpError::NotEnoughRemainingAccounts);
    require!(
        ctx.remaining_accounts.len() == shareholders_len,
        PumpError::NotEnoughRemainingAccounts
    );

    let rent_exempt_minimum = Rent::get()?.try_minimum_balance(0)?;
    let vault_lamports = ctx.accounts.creator_vault.lamports();
    let available = vault_lamports.saturating_sub(rent_exempt_minimum);

    let bonding_curve_creator = ctx.accounts.bonding_curve.creator;
    let signer_seeds: &[&[u8]] = &[
        CREATOR_VAULT_SEED,
        bonding_curve_creator.as_ref(),
        &[creator_vault_bump],
    ];
    let signer: &[&[&[u8]]] = &[signer_seeds];

    let mut distributed = 0u64;
    for i in 0..shareholders_len {
        let shareholder = ctx.accounts.sharing_config.shareholders[i];
        let mut recipient = ctx.remaining_accounts[i];
        require!(
            recipient.address() == shareholder.address,
            PumpError::ShareholderAccountMismatch
        );
        require!(!recipient.is_executable(), PumpError::UnableToDistributeCreatorFeesToExecutableRecipient);

        let amount = if i == shareholders_len - 1 {
            available - distributed
        } else {
            available * shareholder.share_bps as u64 / 10_000
        };
        distributed += amount;

        if amount > 0 {
            // `creator_vault` is never formally `init`'d anywhere in this reimplementation,
            // so its on-chain owner is still the System Program, not this program — a direct
            // `sub_lamports` debit fails (`ExternalAccountLamportSpend`, only the account's
            // real owner may debit it that way). A signed System Program transfer works
            // because it only requires the *source* to be System-owned, which this PDA
            // still is.
            ctx.accounts.system_program.transfer_signed(
                SystemTransferAccounts { from: &mut ctx.accounts.creator_vault, to: &mut recipient },
                amount,
                signer,
            )?;
        }
    }

    let timestamp = unix_timestamp()?;
    let shareholders: Vec<Shareholder> = ctx.accounts.sharing_config.shareholders[..shareholders_len].to_vec();
    emit!(DistributeCreatorFeesEvent {
        timestamp,
        mint: ctx.accounts.mint.address(),
        bonding_curve: ctx.accounts.bonding_curve.address(),
        sharing_config: ctx.accounts.sharing_config.address(),
        admin: ctx.accounts.sharing_config.admin,
        shareholders,
        distributed,
        quote_mint: ctx.accounts.quote_mint.address(),
    });

    msg!("Creator fees successfully distributed");
    Ok(())
}
