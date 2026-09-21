use naclac_lang::prelude::*;
use crate::constants::{CREATOR_VAULT_SEED, PUMP_FEES_PROGRAM_ID};
use crate::errors::PumpError;
use crate::events::CollectCreatorFeeEvent;

// Real accounts (`pump-public-docs/idl/pump.json`): `creator(mut)`,
// `creator_token_account(mut, ATA[creator, quote_token_program, quote_mint])`,
// `creator_vault(mut, pda=[creator-vault, creator])`, `creator_vault_token_account
// (mut, ATA[creator_vault, quote_token_program, quote_mint])`, `quote_mint`,
// `quote_token_program`, `associated_token_program`, `system_program`. No
// signer anywhere — permissionless, confirmed live against real devnet via
// `reference/pump-rust-client/tests/v2_creator_fees.rs::v2_collect_creator_fee`
// (an unrelated payer successfully cranks it). Generic over any SPL quote
// mint/token program, matching the real instruction exactly — not narrowed to
// WSOL, unlike `distribute_creator_fees_v2`'s own deliberate WSOL-only scope
// decision (that narrowing was specific to that instruction, not a blanket
// policy). Same `InterfaceAccount`/`Interface<TokenInterface>` idiom already
// established in `buy_v2.rs`. Real bytecode also rejects a `creator` owned
// by `pump_fees` (a migrated bonding curve's own `sharing_config` address)
// with `UnableToDistributeCreatorVaultMigratedToSharingConfig` — confirmed
// live via `reference/fee-tier-probe/src/bin/probe72.rs`, shares the same
// source file as `collect_creator_fee` (v1) in the real bytecode.
#[instruction_args]
pub struct CollectCreatorFeeV2Args {
    pub creator_vault_bump: u8,
    pub creator_token_account_bump: u8,
    pub creator_vault_token_account_bump: u8,
}

#[derive(Accounts)]
#[instruction(args: CollectCreatorFeeV2Args)]
pub struct CollectCreatorFeeV2 {
    /// SAFETY: only PDA seed material for `creator_vault`/`creator_token_account`
    /// below; never deserialized.
    pub creator: AccountInfo,

    #[account(
        mut,
        associated_token::mint = quote_mint,
        associated_token::authority = creator,
        associated_token::bump = args.creator_token_account_bump,
        token::program = quote_token_program,
    )]
    pub creator_token_account: InterfaceAccount<TokenAccount>,

    /// SAFETY: the `seeds`/`bump` constraint already verifies its address;
    /// it's a lamport-only PDA (no stored data) — only ever a CPI signer
    /// below, never deserialized.
    #[account(
        mut,
        seeds = [CREATOR_VAULT_SEED, creator.address().as_ref()],
        bump = args.creator_vault_bump,
    )]
    pub creator_vault: AccountInfo,

    #[account(
        mut,
        associated_token::mint = quote_mint,
        associated_token::authority = creator_vault,
        associated_token::bump = args.creator_vault_token_account_bump,
        token::program = quote_token_program,
    )]
    pub creator_vault_token_account: InterfaceAccount<TokenAccount>,

    pub quote_mint: InterfaceAccount<Mint>,
    pub quote_token_program: Interface<TokenInterface>,
    pub associated_token_program: Program<AssociatedToken>,
    pub system_program: Program<System>,
}

/// Sweeps `creator_vault_token_account`'s full SPL-token balance to
/// `creator_token_account` — the SPL-token sibling of `collect_creator_fee`
/// (native SOL). Permissionless: anyone may crank it, funds always land at
/// the fixed `creator_vault_token_account` -> `creator_token_account` pair.
/// Unlike the native path there's no rent-exempt floor to preserve (an SPL
/// token account's rent is a fixed lamport reserve separate from its token
/// balance), so the entire token balance is swept.
pub fn collect_creator_fee_v2(
    ctx: Context<CollectCreatorFeeV2>,
    args: CollectCreatorFeeV2Args,
) -> Result<Option<CollectCreatorFeeEvent>> {
    require!(
        ctx.accounts.creator.owner() != PUMP_FEES_PROGRAM_ID,
        PumpError::UnableToDistributeCreatorVaultMigratedToSharingConfig
    );

    let amount = ctx.accounts.creator_vault_token_account.amount();
    if amount == 0 {
        return Ok(None);
    }

    let creator_address = ctx.accounts.creator.address();
    let signer_seeds: &[&[u8]] = &[CREATOR_VAULT_SEED, creator_address.as_ref(), &[args.creator_vault_bump]];
    let signer: &[&[&[u8]]] = &[signer_seeds];

    let quote_decimals = ctx.accounts.quote_mint.decimals();
    ctx.accounts.quote_token_program.transfer_checked_signed(
        TransferCheckedAccounts {
            from: &mut ctx.accounts.creator_vault_token_account,
            mint: &ctx.accounts.quote_mint,
            to: &mut ctx.accounts.creator_token_account,
            authority: &ctx.accounts.creator_vault,
        },
        amount,
        quote_decimals,
        signer,
    )?;

    let timestamp = unix_timestamp()?;
    let event = emit!(CollectCreatorFeeEvent {
        timestamp,
        creator: creator_address,
        creator_fee: amount,
        quote_mint: ctx.accounts.quote_mint.address(),
    });

    msg!("Creator fee successfully collected");
    Ok(Some(event))
}
