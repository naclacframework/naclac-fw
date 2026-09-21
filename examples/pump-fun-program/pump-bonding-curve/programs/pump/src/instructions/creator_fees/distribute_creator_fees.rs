use naclac_lang::prelude::*;
use crate::components::{BondingCurve, Shareholder, SharingConfig};
use crate::constants::{
    BONDING_CURVE_SEED, CREATOR_VAULT_SEED, PUMP_FEES_AUTHORITY_SEED, PUMP_FEES_PROGRAM_ID, SHARING_CONFIG_SEED,
    WSOL_MINT,
};
use crate::errors::PumpError;
use crate::events::DistributeCreatorFeesEvent;

#[derive(Accounts)]
#[instruction(bonding_curve_bump: u8, creator_vault_bump: u8)]
pub struct DistributeCreatorFees {
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

/// Sweeps `creator_vault` down to its rent-exempt minimum, paying the swept
/// amount pro-rata by `share_bps` to `remaining_accounts` — one entry per
/// `sharing_config.shareholders`, in order, bare pubkeys (native SOL only;
/// `_v2` handles non-native quotes). Rounding: every shareholder but the last
/// gets `floor(available * share_bps / 10_000)`; the last gets the exact
/// remainder, so the vault never retains dust above its rent-exempt floor.
/// This rounding rule is a reasonable implementation choice, not verified
/// against the real bytecode's exact behavior for >1 shareholder.
///
/// Requires `bonding_curve.creator == sharing_config`'s own address (real,
/// live-confirmed check, `reference/fee-tier-probe/src/bin/probe71.rs`) --
/// without it, `creator_vault` (derived from `bonding_curve.creator`) and the
/// payout list (`sharing_config.shareholders`, from whichever `sharing_config`
/// the caller passes) would have nothing tying them together, letting anyone
/// redirect any bonding curve's real accumulated fees to an unrelated
/// `sharing_config` they control. Also rejects any executable shareholder
/// recipient (real, live-confirmed check, `reference/fee-tier-probe/src/bin/probe73.rs`)
/// — an executable account can't receive lamports, so a stale shareholder
/// entry that's since become a program account must be removed via
/// `update_fee_shares(_v2)` first rather than silently failing the whole
/// distribution at the transfer step.
pub fn distribute_creator_fees(
    ctx: Context<DistributeCreatorFees>,
    _bonding_curve_bump: u8,
    creator_vault_bump: u8,
) -> Result {
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
        quote_mint: WSOL_MINT,
    });

    msg!("Creator fees successfully distributed");
    Ok(())
}
