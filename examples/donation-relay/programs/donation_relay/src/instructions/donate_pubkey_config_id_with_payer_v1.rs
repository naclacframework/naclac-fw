use naclac_lang::prelude::*;
use crate::components::{Debouncer, EpochTracker};
use crate::constants::{DEBOUNCER_V1_SEED, EPOCH_TRACKER_V1_SEED, WSOL_MINT, MAX_MESSAGE_LEN};
use crate::errors::DonationRelayError;
use crate::events::DonationMadeV1Event;

#[instruction_args]
pub struct DonatePubkeyConfigIdWithPayerV1Args {
    pub amount: u64,
    pub config_id: Address,
    pub tip_bps: u16,
    pub message: ZcString,
    pub credited_to: Address,
    pub epoch_tracker_bump: u8,
    pub debouncer_bump: u8,
    pub from_token_account_bump: u8,
}

// naclac bans on-chain `find_program_address`, so `epoch_tracker`/`debouncer`
// (dynamic on `config_id`/`mint`) need explicit, client-computed bumps.
// `mint` must be declared before both, and `debouncer` before
// `debouncer_token_account`: sibling-field seed references must come after
// the field they reference.
#[derive(Accounts)]
#[instruction(args: DonatePubkeyConfigIdWithPayerV1Args)]
pub struct DonatePubkeyConfigIdWithPayerV1 {
    #[account(mut)]
    pub payer: Signer,

    #[account(mut)]
    pub from: Signer,

    #[account(mut, address = WSOL_MINT)]
    pub mint: Account<Mint>,

    #[account(
        init_if_needed,
        payer = payer,
        seeds = [EPOCH_TRACKER_V1_SEED, args.config_id.as_ref(), mint.address().as_ref()],
        bump = args.epoch_tracker_bump,
    )]
    pub epoch_tracker: Account<EpochTracker>,

    #[account(
        init_if_needed,
        payer = payer,
        seeds = [DEBOUNCER_V1_SEED, args.config_id.as_ref(), mint.address().as_ref()],
        bump = args.debouncer_bump,
    )]
    pub debouncer: Account<Debouncer>,

    /// SAFETY: may not exist yet — created idempotently by the
    /// `init_if_needed`/`associated_token::mint`/`::authority` constraint
    /// below (a no-op if it already exists), then reinterpreted as a
    /// `TokenAccount` in the instruction body.
    #[account(
        mut,
        init_if_needed,
        payer = payer,
        associated_token::mint = mint,
        associated_token::authority = debouncer,
    )]
    pub debouncer_token_account: AccountInfo,

    #[account(
        mut,
        associated_token::mint = mint,
        associated_token::authority = from,
        associated_token::bump = args.from_token_account_bump,
    )]
    pub from_token_account: Account<TokenAccount>,

    /// SAFETY: dead in this scoped WSOL-only pass — the real program reads
    /// this to bypass its own mint-safety check for non-whitelisted mints,
    /// but WSOL already passes that check unconditionally (confirmed via
    /// `reference/donation-relay-probe/src/bin/probe2.rs`), so this
    /// reimplementation never needs to read it.
    pub mint_whitelist: AccountInfo,

    pub associated_token_program: Program<AssociatedToken>,
    pub token_program: Program<Token>,
    pub system_program: Program<System>,
}

pub fn donate_pubkey_config_id_with_payer_v1(
    ctx: Context<DonatePubkeyConfigIdWithPayerV1>,
    args: DonatePubkeyConfigIdWithPayerV1Args,
) -> Result {
    // Real on-chain limit confirmed via
    // `reference/donation-relay-probe/src/bin/probe3.rs` (binary search).
    require!(
        args.message.len() <= MAX_MESSAGE_LEN,
        DonationRelayError::InvalidMessageLength
    );

    let mint_addr = ctx.accounts.mint.address();

    let epoch_tracker = &mut ctx.accounts.epoch_tracker;
    if epoch_tracker.state == 0 {
        epoch_tracker.bump = args.epoch_tracker_bump;
        epoch_tracker.state = 1;
        epoch_tracker.config_id = args.config_id;
        epoch_tracker.mint = mint_addr;
        epoch_tracker.current_epoch = 0;
    }

    let debouncer = &mut ctx.accounts.debouncer;
    if debouncer.state == 0 {
        debouncer.bump = args.debouncer_bump;
        debouncer.state = 1;
        debouncer.config_id = args.config_id;
        debouncer.mint = mint_addr;
        debouncer.total_amount = 0;
    }

    let mut debouncer_token_account =
        Account::<TokenAccount>::try_from_mut(&ctx.accounts.debouncer_token_account, 0)?;

    // Full amount is transferred — confirmed via probe1 that `tip` is
    // bookkeeping only and never deducted on-chain.
    let mint_decimals = ctx.accounts.mint.decimals();
    ctx.accounts.token_program.transfer_checked(
        TransferCheckedAccounts {
            from: &mut ctx.accounts.from_token_account,
            mint: &ctx.accounts.mint,
            to: &mut debouncer_token_account,
            authority: &ctx.accounts.from,
        },
        args.amount,
        mint_decimals,
    )?;

    // Accumulates across donations to the same `(config_id, mint)` pair —
    // confirmed via probe4.
    let debouncer = &mut ctx.accounts.debouncer;
    debouncer.total_amount = debouncer
        .total_amount
        .checked_add(args.amount)
        .ok_or(DonationRelayError::MathOverflow)?;

    let tip = (args.amount as u128 * args.tip_bps as u128 / 10_000) as u64;

    emit!(DonationMadeV1Event {
        config_id: args.config_id,
        mint: mint_addr,
        gross_amount: args.amount,
        tip,
        message: String::from(args.message.as_str()),
        credited_to: args.credited_to,
    });

    Ok(())
}
