use naclac_lang::prelude::*;
use pump_fees_client::instructions::GetFeesCpiAccounts;
use pump_fees_client::PumpFees;
use crate::components::{BondingCurve, Global, GlobalVolumeAccumulator, UserVolumeAccumulator};
use crate::constants::{
    BONDING_CURVE_SEED, BUYBACK_VAULT_SEED, CREATOR_VAULT_SEED, FEE_CONFIG_SEED, GLOBAL_SEED,
    GLOBAL_VOLUME_ACCUMULATOR_SEED, PUMP_AUTHORITY_SEED, PUMP_FEES_PROGRAM_ID, USER_VOLUME_ACCUMULATOR_SEED,
};
use crate::errors::PumpError;
use crate::events::{CompleteEvent, TradeEvent};
use crate::systems::{
    bonding_curve_market_cap_lamports, fee_amount_ceil, fee_amount_floor, get_fees_via_cpi,
    tokens_out_for_net_sol_exact_in, GetFeesParams,
};

// Real `buy_exact_sol_in` account list (confirmed via a real, successful
// mainnet transaction — `docs/plan/bonding-curve-05-batch1-v2-instructions.md`)
// has no `sharing_config`/`bonding_curve_v2` account in its documented 16, but
// DOES need `bonding_curve_v2` (readonly) then `buyback_fee_recipient`
// (writable) appended as undocumented remaining accounts, same as classic
// `buy`. `min_tokens_out` must be nonzero — real `pump.so` rejects `0`
// outright (confirmed via probing), not just returned to a "no slippage
// protection" special case.
#[instruction_args]
pub struct BuyExactSolInArgs {
    pub spendable_sol_in: u64,
    pub min_tokens_out: u64,
    pub bonding_curve_bump: u8,
    pub associated_bonding_curve_bump: u8,
    pub associated_user_bump: u8,
    pub creator_vault_bump: u8,
    pub user_volume_accumulator_bump: u8,
    pub fee_config_bump: u8,
    pub buyback_index: u8,
    pub buyback_vault_bump: u8,
}

#[derive(Accounts)]
#[instruction(args: BuyExactSolInArgs)]
pub struct BuyExactSolIn {
    #[account(seeds = [GLOBAL_SEED], bump)]
    pub global: Account<Global>,

    /// SAFETY: validated in the handler body against
    /// `{global.fee_recipient} ∪ {global.fee_recipients}` (pool membership,
    /// not a single fixed address — no declarative constraint supports OR);
    /// never deserialized, only a lamport destination below.
    #[account(mut)]
    pub fee_recipient: AccountInfo,

    pub mint: InterfaceAccount<Mint>,

    #[account(
        mut,
        seeds = [BONDING_CURVE_SEED, mint.address().as_ref()],
        bump = args.bonding_curve_bump,
    )]
    pub bonding_curve: Account<BondingCurve>,

    #[account(
        mut,
        associated_token::mint = mint,
        associated_token::authority = bonding_curve,
        associated_token::bump = args.associated_bonding_curve_bump,
    )]
    pub associated_bonding_curve: InterfaceAccount<TokenAccount>,

    #[account(mut)]
    pub user: Signer,

    #[account(
        mut,
        associated_token::mint = mint,
        associated_token::authority = user,
        associated_token::bump = args.associated_user_bump,
    )]
    pub associated_user: InterfaceAccount<TokenAccount>,

    pub system_program: Program<System>,
    pub token_program: Interface<TokenInterface>,

    /// SAFETY: the `seeds`/`bump` constraint already verifies its address;
    /// it's a lamport-only PDA (no stored data), never `init`'d so still
    /// System-owned — only ever a lamport destination below via a plain
    /// System Program transfer from `user`, never deserialized.
    #[account(
        mut,
        seeds = [CREATOR_VAULT_SEED, bonding_curve.creator.as_ref()],
        bump = args.creator_vault_bump,
    )]
    pub creator_vault: AccountInfo,

    /// SAFETY: self-reference, unused beyond seed material for `fee_config`
    /// below — naclac's `emit!` needs no `event_authority`/self-CPI account,
    /// unlike Anchor's own event-emission convention.
    #[account(address = crate::ID)]
    pub program: AccountInfo,

    /// Real `buy_exact_sol_in` never writes this account (confirmed: classic
    /// `buy`/`sell` never touch it either, per `probe16`'s findings in
    /// `fees-07-donation-relay-progress.md`) — must already exist (created by
    /// an earlier classic `buy` at least once), never `init_if_needed` here,
    /// matching the real IDL marking it non-writable for this instruction
    /// specifically.
    #[account(seeds = [GLOBAL_VOLUME_ACCUMULATOR_SEED], bump)]
    pub global_volume_accumulator: Account<GlobalVolumeAccumulator>,

    #[account(
        mut,
        init_if_needed,
        payer = user,
        seeds = [USER_VOLUME_ACCUMULATOR_SEED, user.address().as_ref()],
        bump = args.user_volume_accumulator_bump,
    )]
    pub user_volume_accumulator: Account<UserVolumeAccumulator>,

    /// SAFETY: the `seeds`/`owner` constraints fully validate this; only
    /// passed as a CPI account to `pump_fees::get_fees` below, never
    /// deserialized here.
    #[account(
        seeds = [FEE_CONFIG_SEED, program.address().as_ref()],
        seeds::program = PUMP_FEES_PROGRAM_ID,
        bump = args.fee_config_bump,
        owner = PUMP_FEES_PROGRAM_ID,
    )]
    pub fee_config: AccountInfo,

    pub fee_program: Program<PumpFees>,

    /// SAFETY: unused — this reimplementation has no v2 bonding-curve concept.
    pub bonding_curve_v2: AccountInfo,

    /// SAFETY: the `seeds`/`owner` constraints fully validate this as a real
    /// `pump_fees::BuybackVault` PDA; `buyback_index` is caller-supplied —
    /// every index 0..8 is an equally valid, protocol-owned vault, so that
    /// constraint alone is the whole security boundary, never deserialized.
    #[account(
        mut,
        seeds = [BUYBACK_VAULT_SEED, &[args.buyback_index]],
        seeds::program = PUMP_FEES_PROGRAM_ID,
        bump = args.buyback_vault_bump,
        owner = PUMP_FEES_PROGRAM_ID,
    )]
    pub buyback_fee_recipient: AccountInfo,

    /// SAFETY: the `seeds`/`bump` constraint already verifies its address;
    /// it's a lamport-only PDA (no stored data) — only ever used as the
    /// signed-CPI proof-of-origin for `pump_fees::get_fees` below.
    #[account(seeds = [PUMP_AUTHORITY_SEED], bump)]
    pub pump_authority: AccountInfo,
}

#[instruction]
pub fn buy_exact_sol_in(ctx: Context<BuyExactSolIn>, args: BuyExactSolInArgs) -> Result {
    require!(args.spendable_sol_in > 0, PumpError::BuyZeroAmount);
    require!(args.min_tokens_out > 0, PumpError::ZeroMinTokensOut);

    let fee_recipient_address = ctx.accounts.fee_recipient.address();
    require!(
        fee_recipient_address == ctx.accounts.global.fee_recipient
            || ctx.accounts.global.fee_recipients.contains(&fee_recipient_address),
        PumpError::InvalidFeeRecipient
    );
    require!(
        ctx.accounts.global.buyback_fee_recipients.contains(&ctx.accounts.buyback_fee_recipient.address()),
        PumpError::InvalidBuybackFeeRecipient
    );

    let virtual_token_reserves = ctx.accounts.bonding_curve.virtual_token_reserves;
    let virtual_sol_reserves = ctx.accounts.bonding_curve.virtual_quote_reserves;
    let token_total_supply = ctx.accounts.bonding_curve.token_total_supply;

    // Confirmed real (unlike classic `buy`, which passes a hardcoded 0 here)
    // via `probe41.rs` against real, deployed `pump.so`.
    let market_cap_lamports = bonding_curve_market_cap_lamports(
        virtual_sol_reserves as u128,
        token_total_supply as u128,
        virtual_token_reserves as u128,
    )?;

    let fees = get_fees_via_cpi(
        &ctx.accounts.fee_program,
        GetFeesCpiAccounts {
            config_program_id: ctx.accounts.program.to_cpi_handle(),
            fee_config: ctx.accounts.fee_config.to_cpi_handle(),
            pump_authority: ctx.accounts.pump_authority.to_cpi_handle(),
        },
        ctx.bumps.pump_authority,
        GetFeesParams {
            is_pump_pool: Bool::from(true),
            market_cap_lamports,
            // Confirmed real via a decoded, successful mainnet transaction
            // (`inspect_buy_exact_sol_in_tx.rs`): `trade_size_lamports` here
            // is `spendable_sol_in`, not any derived `net_sol` estimate.
            trade_size_lamports: args.spendable_sol_in,
            is_new_quote_mint: Bool::from(false),
        },
    )?;

    let has_creator = ctx.accounts.bonding_curve.creator != Address::default();
    let effective_creator_fee_bps = if has_creator { fees.creator_fee_bps } else { 0 };
    let total_fee_bps = fees.protocol_fee_bps.checked_add(effective_creator_fee_bps).ok_or(PumpError::MathOverflow)?;

    // Real IDL-documented 4-step quote formula (verbatim, confirmed via
    // `probe41.rs`'s successful end-to-end replay against real `pump.so`).
    let mut net_sol = (args.spendable_sol_in as u128)
        .checked_mul(10_000)
        .ok_or(PumpError::MathOverflow)?
        .checked_div(10_000u128.checked_add(total_fee_bps as u128).ok_or(PumpError::MathOverflow)?)
        .ok_or(PumpError::MathOverflow)? as u64;

    let protocol_fee = fee_amount_ceil(net_sol as u128, fees.protocol_fee_bps)? as u64;
    let creator_fee_estimate = fee_amount_ceil(net_sol as u128, effective_creator_fee_bps)? as u64;
    let fees_total = protocol_fee.checked_add(creator_fee_estimate).ok_or(PumpError::MathOverflow)?;

    if let Some(over) = (net_sol.checked_add(fees_total).ok_or(PumpError::MathOverflow)?)
        .checked_sub(args.spendable_sol_in)
        .filter(|&o| o > 0)
    {
        net_sol = net_sol.checked_sub(over).ok_or(PumpError::MathOverflow)?;
    }

    require!(net_sol > 0, PumpError::BuyZeroAmount);
    let tokens_out = tokens_out_for_net_sol_exact_in(
        (net_sol - 1) as u128,
        virtual_token_reserves as u128,
        (virtual_sol_reserves as u128).checked_add((net_sol - 1) as u128).ok_or(PumpError::MathOverflow)?,
    )? as u64;
    require!(tokens_out > 0, PumpError::BuyZeroAmount);
    require!(tokens_out >= args.min_tokens_out, PumpError::SlippageExceeded);
    require!(tokens_out <= ctx.accounts.bonding_curve.real_token_reserves, PumpError::NotEnoughTokensToBuy);

    // Recompute the final fee split against the (possibly step-3-adjusted) `net_sol`.
    let protocol_fee = fee_amount_ceil(net_sol as u128, fees.protocol_fee_bps)? as u64;
    let creator_fee = if has_creator { fee_amount_ceil(net_sol as u128, fees.creator_fee_bps)? as u64 } else { 0 };

    let buyback_share = fee_amount_floor(protocol_fee as u128, ctx.accounts.global.buyback_basis_points)? as u64;
    let fee_recipient_share = protocol_fee.checked_sub(buyback_share).ok_or(PumpError::MathOverflow)?;

    ctx.accounts.system_program.transfer(
        SystemTransferAccounts { from: &mut ctx.accounts.user, to: &mut ctx.accounts.bonding_curve },
        net_sol,
    )?;
    if fee_recipient_share > 0 {
        ctx.accounts.system_program.transfer(
            SystemTransferAccounts { from: &mut ctx.accounts.user, to: &mut ctx.accounts.fee_recipient },
            fee_recipient_share,
        )?;
    }
    if buyback_share > 0 {
        ctx.accounts.system_program.transfer(
            SystemTransferAccounts { from: &mut ctx.accounts.user, to: &mut ctx.accounts.buyback_fee_recipient },
            buyback_share,
        )?;
    }

    let is_cashback_coin: bool = ctx.accounts.bonding_curve.is_cashback_coin.into();
    if creator_fee > 0 {
        if is_cashback_coin {
            ctx.accounts.system_program.transfer(
                SystemTransferAccounts { from: &mut ctx.accounts.user, to: &mut ctx.accounts.user_volume_accumulator },
                creator_fee,
            )?;
            let user_volume_accumulator = &mut ctx.accounts.user_volume_accumulator;
            user_volume_accumulator.cashback_earned =
                user_volume_accumulator.cashback_earned.checked_add(creator_fee).ok_or(PumpError::MathOverflow)?;
        } else {
            ctx.accounts.system_program.transfer(
                SystemTransferAccounts { from: &mut ctx.accounts.user, to: &mut ctx.accounts.creator_vault },
                creator_fee,
            )?;
        }
    }

    let mint_address = ctx.accounts.mint.address();
    let signer_seeds: &[&[u8]] = &[BONDING_CURVE_SEED, mint_address.as_ref(), &[args.bonding_curve_bump]];
    let signer: &[&[&[u8]]] = &[signer_seeds];
    let decimals = ctx.accounts.mint.decimals();
    ctx.accounts.token_program.transfer_checked_signed(
        TransferCheckedAccounts {
            from: &mut ctx.accounts.associated_bonding_curve,
            mint: &ctx.accounts.mint,
            to: &mut ctx.accounts.associated_user,
            authority: &ctx.accounts.bonding_curve,
        },
        tokens_out,
        decimals,
        signer,
    )?;

    let bonding_curve = &mut ctx.accounts.bonding_curve;
    bonding_curve.virtual_token_reserves =
        bonding_curve.virtual_token_reserves.checked_sub(tokens_out).ok_or(PumpError::MathOverflow)?;
    bonding_curve.virtual_quote_reserves =
        bonding_curve.virtual_quote_reserves.checked_add(net_sol).ok_or(PumpError::MathOverflow)?;
    bonding_curve.real_token_reserves =
        bonding_curve.real_token_reserves.checked_sub(tokens_out).ok_or(PumpError::MathOverflow)?;
    bonding_curve.real_quote_reserves =
        bonding_curve.real_quote_reserves.checked_add(net_sol).ok_or(PumpError::MathOverflow)?;
    if bonding_curve.real_token_reserves == 0 {
        bonding_curve.complete = true.into();
    }

    let timestamp = unix_timestamp()?;
    let user_address = ctx.accounts.user.address();
    let creator = ctx.accounts.bonding_curve.creator;
    let complete: bool = ctx.accounts.bonding_curve.complete.into();
    let cashback_fee_basis_points = if is_cashback_coin { fees.creator_fee_bps } else { 0 };
    let bonding_curve = &ctx.accounts.bonding_curve;
    emit!(TradeEvent {
        mint: mint_address,
        sol_amount: net_sol,
        token_amount: tokens_out,
        is_buy: Bool::from(true),
        user: user_address,
        timestamp,
        virtual_sol_reserves: bonding_curve.virtual_quote_reserves,
        virtual_token_reserves: bonding_curve.virtual_token_reserves,
        real_sol_reserves: bonding_curve.real_quote_reserves,
        real_token_reserves: bonding_curve.real_token_reserves,
        fee_recipient: ctx.accounts.fee_recipient.address(),
        fee_basis_points: fees.protocol_fee_bps,
        fee: protocol_fee,
        creator,
        creator_fee_basis_points: fees.creator_fee_bps,
        creator_fee,
        track_volume: Bool::from(false),
        total_unclaimed_tokens: ctx.accounts.user_volume_accumulator.total_unclaimed_tokens,
        total_claimed_tokens: ctx.accounts.user_volume_accumulator.total_claimed_tokens,
        current_sol_volume: ctx.accounts.user_volume_accumulator.current_sol_volume,
        last_update_timestamp: ctx.accounts.user_volume_accumulator.last_update_timestamp,
        ix_name: String::from("buy_exact_sol_in"),
        mayhem_mode: bonding_curve.is_mayhem_mode,
        cashback_fee_basis_points,
        cashback: if is_cashback_coin { creator_fee } else { 0 },
        buyback_fee_basis_points: ctx.accounts.global.buyback_basis_points,
        buyback_fee: buyback_share,
        shareholders: Vec::new(),
        quote_mint: bonding_curve.quote_mint,
        quote_amount: net_sol,
        virtual_quote_reserves: bonding_curve.virtual_quote_reserves,
        real_quote_reserves: bonding_curve.real_quote_reserves,
    });

    if complete {
        emit!(CompleteEvent {
            user: user_address,
            mint: mint_address,
            bonding_curve: ctx.accounts.bonding_curve.address(),
            timestamp,
            quote_mint: ctx.accounts.bonding_curve.quote_mint,
        });
    }

    msg!("Buy successful");
    Ok(())
}
