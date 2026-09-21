use naclac_lang::prelude::*;
use donation_relay_client::instructions::{
    DonatePubkeyConfigIdWithPayerV1Cpi, DonatePubkeyConfigIdWithPayerV1CpiAccounts,
};
// `pump_fees` depends on `donation-relay-client` both as a normal `cpi`
// dependency and (via `[dev-dependencies]`, for its own offchain-built
// tests) as an `offchain` one, unifying both features on the same crate —
// the `Cpi`-suffixed name disambiguates which shape this is, since the
// plain name only exists when a build activates `cpi` alone.
use donation_relay_client::types::DonatePubkeyConfigIdWithPayerV1ArgsCpi;
use donation_relay_client::DonationRelay;
use crate::components::{DonationFeePda, FeeProgramGlobal};
use crate::constants::{
    DEBOUNCER_V1, DONATION_FEE_PDA_SEED, DONATION_RELAY_PROGRAM_ID, EPOCH_TRACKER_V1,
    FEE_PROGRAM_GLOBAL_SEED, WSOL_MINT,
};
use crate::errors::FeesError;
use crate::events::DonationFeePdaCranked;

// naclac forbids on-chain PDA search, so every dynamic PDA below (everything
// except `fee_program_global`, whose seed is fully literal) takes an
// explicit caller-supplied bump — same pattern as every other instruction in
// this program. `donation_fee_pda`'s own bump comes from its already-stored
// `bump` field instead of an arg, since the account already exists.
#[instruction_args]
pub struct CrankDonationFeePdaArgs {
    pub donation_fee_pda_ata_bump: u8,
    pub epoch_tracker_bump: u8,
    pub debouncer_bump: u8,
    pub debouncer_ata_bump: u8,
}

// Confirmed empirically (`reference/fee-tier-probe/src/bin/probe38.rs`
// against real, deployed `pump_fees.so`/`donation_relay.so`) before writing
// this: sweeps `donation_fee_pda`'s full accumulated WSOL balance —
// `donation_fee_pda_ata`'s token balance plus `donation_fee_pda`'s own
// lamports above its rent-exempt minimum — into `donation_relay` via a
// signed CPI into `donate_pubkey_config_id_with_payer_v1`, with
// `donation_fee_pda` itself as the PDA-signed donor (`from`). The lamport
// excess is a raw (`sub_lamports`/`add_lamports`) write reconciled by
// `sync_native_with_extra_accounts`, mirroring `pump_amm::migrate`'s own
// `pool_migration_fee` sweep. `message` is `base58(base_mint) + "," +
// base58(creator)`; `credited_to` is this program's own ID — confirmed via
// the same probe, crank-relayed donations attribute to `pump_fees` itself,
// not `donation_fee_pda.creator`. `tip_bps` is confirmed `0`.
#[derive(Accounts)]
#[instruction(args: CrankDonationFeePdaArgs)]
pub struct CrankDonationFeePda {
    #[account(mut)]
    pub payer: Signer,

    pub system_program: Program<System>,
    pub token_program: Program<Token>,
    pub associated_token_program: Program<AssociatedToken>,

    /// SAFETY: address pinned via `address = RENT_SYSVAR_ID`; passed to the
    /// manual `sync_native_with_extra_accounts` call below, whose second
    /// account slot the real classic-Token `SyncNative` processor
    /// specifically requires to be the Rent sysvar.
    #[account(address = RENT_SYSVAR_ID)]
    pub rent: AccountInfo,

    #[account(seeds = [FEE_PROGRAM_GLOBAL_SEED])]
    pub fee_program_global: Account<FeeProgramGlobal>,

    /// SAFETY: only used as PDA seed material below; not deserialized.
    pub base_mint: AccountInfo,

    /// SAFETY: plain address material distinguishing donation campaigns;
    /// not a signer, never deserialized.
    pub config_id: AccountInfo,

    #[account(
        mut,
        seeds = [DONATION_FEE_PDA_SEED, base_mint.address().as_ref(), config_id.address().as_ref()],
        bump = donation_fee_pda.bump,
    )]
    pub donation_fee_pda: Account<DonationFeePda>,

    #[account(mut, address = WSOL_MINT)]
    pub quote_mint: Account<Mint>,

    #[account(
        mut,
        init_if_needed,
        payer = payer,
        associated_token::mint = quote_mint,
        associated_token::authority = donation_fee_pda,
        token::program = token_program,
    )]
    pub donation_fee_pda_ata: Account<TokenAccount>,

    pub donation_relay_program: Program<DonationRelay>,

    /// SAFETY: dead — real `donation_relay` bypasses its own mint-safety
    /// check for WSOL unconditionally; kept for account-shape parity only.
    pub mint_whitelist: AccountInfo,

    /// SAFETY: owned by `donation_relay_program`; only ever passed through
    /// as a CPI handle below, never deserialized here.
    #[account(
        mut,
        seeds = [EPOCH_TRACKER_V1, config_id.address().as_ref(), quote_mint.address().as_ref()],
        seeds::program = DONATION_RELAY_PROGRAM_ID,
        bump = args.epoch_tracker_bump,
    )]
    pub epoch_tracker: AccountInfo,

    /// SAFETY: same as `epoch_tracker` above.
    #[account(
        mut,
        seeds = [DEBOUNCER_V1, config_id.address().as_ref(), quote_mint.address().as_ref()],
        seeds::program = DONATION_RELAY_PROGRAM_ID,
        bump = args.debouncer_bump,
    )]
    pub debouncer: AccountInfo,

    /// SAFETY: `debouncer`'s own WSOL ATA; created idempotently by the
    /// nested CPI itself, only passed through as a CPI handle here.
    #[account(
        mut,
        seeds = [debouncer.address().as_ref(), token_program.address().as_ref(), quote_mint.address().as_ref()],
        seeds::program = ASSOCIATED_TOKEN_PROGRAM_ID,
        bump = args.debouncer_ata_bump,
    )]
    pub debouncer_ata: AccountInfo,
}

pub fn crank_donation_fee_pda(
    ctx: Context<CrankDonationFeePda>,
    args: CrankDonationFeePdaArgs,
) -> Result<Option<DonationFeePdaCranked>> {
    let data_len = 8 + core::mem::size_of::<DonationFeePda>();
    let rent_exempt_minimum = Rent::get()?.try_minimum_balance(data_len)?;
    let donation_fee_pda_lamports = ctx.accounts.donation_fee_pda.to_account_info().lamports();
    let lamport_excess = donation_fee_pda_lamports.saturating_sub(rent_exempt_minimum);

    let ata_balance_before = ctx.accounts.donation_fee_pda_ata.amount();
    let amount = ata_balance_before
        .checked_add(lamport_excess)
        .ok_or(FeesError::MathOverflow)?;

    if amount == 0 {
        return Ok(None);
    }

    if lamport_excess > 0 {
        ctx.accounts.donation_fee_pda.sub_lamports(lamport_excess)?;
        ctx.accounts.donation_fee_pda_ata.add_lamports(lamport_excess)?;
    }

    let donation_fee_pda_extra: [CpiHandle<'_>; 1] = [ctx.accounts.donation_fee_pda.to_cpi_handle()];
    naclac_lang::token::sync_native_with_extra_accounts(
        ctx.accounts.token_program.to_cpi_handle(),
        ctx.accounts.donation_fee_pda_ata.to_cpi_handle_mut(),
        ctx.accounts.rent.to_cpi_handle(),
        &donation_fee_pda_extra,
    )?;

    let base_mint_addr = ctx.accounts.base_mint.address();
    let creator = ctx.accounts.donation_fee_pda.creator;

    let mut base_mint_b58 = [0u8; 44];
    let base_mint_len = base58::encode_32(&base_mint_addr, &mut base_mint_b58) as usize;
    let mut creator_b58 = [0u8; 44];
    let creator_len = base58::encode_32(&creator, &mut creator_b58) as usize;

    let mut message_buf = [0u8; 89];
    message_buf[..base_mint_len].copy_from_slice(&base_mint_b58[..base_mint_len]);
    message_buf[base_mint_len] = b',';
    message_buf[base_mint_len + 1..base_mint_len + 1 + creator_len]
        .copy_from_slice(&creator_b58[..creator_len]);
    let message_len = base_mint_len + 1 + creator_len;
    let message = ZcString::from_bytes(&message_buf[..message_len])?;

    let config_id_addr = ctx.accounts.config_id.address();
    let donation_fee_pda_addr = ctx.accounts.donation_fee_pda.address();
    let donation_fee_pda_bump = ctx.accounts.donation_fee_pda.bump;

    let donation_fee_pda_signer_seeds: &[&[u8]] = &[
        &DONATION_FEE_PDA_SEED,
        base_mint_addr.as_ref(),
        config_id_addr.as_ref(),
        &[donation_fee_pda_bump],
    ];
    let donation_fee_pda_signer: &[&[&[u8]]] = &[donation_fee_pda_signer_seeds];

    ctx.accounts.donation_relay_program.donate_pubkey_config_id_with_payer_v1_signed(
        DonatePubkeyConfigIdWithPayerV1CpiAccounts {
            payer: ctx.accounts.payer.to_cpi_handle_mut(),
            from: ctx.accounts.donation_fee_pda.to_cpi_handle_mut(),
            mint: ctx.accounts.quote_mint.to_cpi_handle_mut(),
            epoch_tracker: ctx.accounts.epoch_tracker.to_cpi_handle_mut(),
            debouncer: ctx.accounts.debouncer.to_cpi_handle_mut(),
            debouncer_token_account: ctx.accounts.debouncer_ata.to_cpi_handle_mut(),
            from_token_account: ctx.accounts.donation_fee_pda_ata.to_cpi_handle_mut(),
            mint_whitelist: ctx.accounts.mint_whitelist.to_cpi_handle(),
            associated_token_program: ctx.accounts.associated_token_program.to_cpi_handle(),
            token_program: ctx.accounts.token_program.to_cpi_handle(),
            system_program: ctx.accounts.system_program.to_cpi_handle(),
        },
        DonatePubkeyConfigIdWithPayerV1ArgsCpi {
            amount,
            config_id: config_id_addr,
            tip_bps: 0,
            message,
            credited_to: crate::ID,
            epoch_tracker_bump: args.epoch_tracker_bump,
            debouncer_bump: args.debouncer_bump,
            from_token_account_bump: args.donation_fee_pda_ata_bump,
        },
        donation_fee_pda_signer,
    )?;

    let now = unix_timestamp()?;
    let donation_fee_pda = &mut ctx.accounts.donation_fee_pda;
    donation_fee_pda.total_donated = donation_fee_pda
        .total_donated
        .checked_add(amount)
        .ok_or(FeesError::MathOverflow)?;
    donation_fee_pda.last_crank_ts = now;

    let event = emit!(DonationFeePdaCranked {
        timestamp: now,
        amount,
        signer: ctx.accounts.payer.address(),
        donation_fee_pda: donation_fee_pda_addr,
        config_id: config_id_addr,
        base_mint: base_mint_addr,
        quote_mint: WSOL_MINT,
        creator,
    });

    Ok(Some(event))
}
