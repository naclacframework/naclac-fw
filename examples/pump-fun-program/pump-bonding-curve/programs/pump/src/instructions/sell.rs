use naclac_lang::prelude::*;
use pump_fees_client::instructions::GetFeesCpiAccounts;
use pump_fees_client::PumpFees;
use crate::components::{BondingCurve, Global, UserVolumeAccumulator};
use crate::constants::{
    BONDING_CURVE_SEED, BONDING_CURVE_V2_SEED, BUYBACK_VAULT_SEED, CREATOR_VAULT_RENT_EXEMPT_MINIMUM,
    CREATOR_VAULT_SEED, FEE_CONFIG_SEED, GLOBAL_SEED, PUMP_AUTHORITY_SEED, PUMP_FEES_PROGRAM_ID,
    USER_VOLUME_ACCUMULATOR_SEED,
};
use crate::errors::PumpError;
use crate::events::TradeEvent;
use crate::systems::{fee_amount_ceil, fee_amount_floor, get_fees_via_cpi, gross_sol_for_tokens_sell, GetFeesParams};

// Classic `sell`. `amount` is the exact token amount sold; `min_sol_output`
// is the caller's slippage floor on the net SOL received. Cashback's
// `user_volume_accumulator` is included unconditionally (rather than as an
// optional remaining account), matching `buy`'s own treatment.
#[instruction_args]
pub struct SellArgs {
    pub amount: u64,
    pub min_sol_output: u64,
    pub bonding_curve_bump: u8,
    pub associated_bonding_curve_bump: u8,
    pub associated_user_bump: u8,
    pub creator_vault_bump: u8,
    pub user_volume_accumulator_bump: u8,
    pub fee_config_bump: u8,
    pub buyback_index: u8,
    pub buyback_vault_bump: u8,
    pub bonding_curve_v2_bump: u8,
}

#[derive(Accounts)]
#[instruction(args: SellArgs)]
pub struct Sell {
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

    // Must be declared before any sibling field whose constraint references it.
    #[account(mut)]
    pub user: Signer,

    #[account(
        mut,
        associated_token::mint = mint,
        associated_token::authority = bonding_curve,
        associated_token::bump = args.associated_bonding_curve_bump,
    )]
    pub associated_bonding_curve: InterfaceAccount<TokenAccount>,

    #[account(
        mut,
        associated_token::mint = mint,
        associated_token::authority = user,
        associated_token::bump = args.associated_user_bump,
    )]
    pub associated_user: InterfaceAccount<TokenAccount>,

    pub token_program: Interface<TokenInterface>,
    pub system_program: Program<System>,

    /// SAFETY: the `seeds`/`bump` constraint already verifies its address;
    /// it's a lamport-only PDA (no stored data), never `init`'d so still
    /// System-owned. `creator_fee` itself is paid via raw `add_lamports`
    /// (sourced from `bonding_curve`'s own debit, no CPI needed for either
    /// this program-owned account). Its rent-exempt top-up, when needed, is
    /// a separate real `System::Transfer` CPI from `user` instead — real,
    /// confirmed via `reference/fee-tier-probe/src/bin/probe54.rs` against
    /// real deployed bytecode (unlike the fee itself, the top-up is not
    /// sourced from the trade's own proceeds).
    #[account(
        mut,
        seeds = [CREATOR_VAULT_SEED, bonding_curve.creator.as_ref()],
        bump = args.creator_vault_bump,
    )]
    pub creator_vault: AccountInfo,

    /// SAFETY: self-reference, unused beyond seed material for `fee_config` below.
    #[account(address = crate::ID)]
    pub program: AccountInfo,

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

    /// SAFETY: real `pump.so` validates this address even though the
    /// account never needs to exist (real transactions pass it at 0
    /// lamports for a classic, non-mayhem coin) — confirmed via
    /// `reference/fee-tier-probe/src/bin/probe52.rs`/`probe53.rs` against
    /// real deployed bytecode and a real mainnet transaction. Never
    /// deserialized; this reimplementation has no v2 bonding-curve concept
    /// beyond satisfying this address check.
    #[account(
        seeds = [BONDING_CURVE_V2_SEED, mint.address().as_ref()],
        bump = args.bonding_curve_v2_bump,
    )]
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
pub fn sell(ctx: Context<Sell>, args: SellArgs) -> Result {
    require!(args.amount > 0, PumpError::MathOverflow);
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

    let gross_sol = gross_sol_for_tokens_sell(
        args.amount as u128,
        virtual_sol_reserves as u128,
        virtual_token_reserves as u128,
    )? as u64;

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
            market_cap_lamports: 0,
            trade_size_lamports: gross_sol,
            is_new_quote_mint: Bool::from(false),
        },
    )?;

    let creator_fee = if ctx.accounts.bonding_curve.creator == Address::default() {
        0
    } else {
        fee_amount_ceil(gross_sol as u128, fees.creator_fee_bps)? as u64
    };
    let protocol_fee = fee_amount_ceil(gross_sol as u128, fees.protocol_fee_bps)? as u64;

    let total_fees = protocol_fee.checked_add(creator_fee).ok_or(PumpError::MathOverflow)?;
    let sol_output = gross_sol.checked_sub(total_fees).ok_or(PumpError::MathOverflow)?;
    require!(sol_output >= args.min_sol_output, PumpError::SlippageExceeded);

    let buyback_share = fee_amount_floor(protocol_fee as u128, ctx.accounts.global.buyback_basis_points)? as u64;
    let fee_recipient_share = protocol_fee.checked_sub(buyback_share).ok_or(PumpError::MathOverflow)?;

    let decimals = ctx.accounts.mint.decimals();
    ctx.accounts.token_program.transfer_checked(
        TransferCheckedAccounts {
            from: &mut ctx.accounts.associated_user,
            mint: &ctx.accounts.mint,
            to: &mut ctx.accounts.associated_bonding_curve,
            authority: &ctx.accounts.user,
        },
        args.amount,
        decimals,
    )?;

    ctx.accounts.bonding_curve.sub_lamports(gross_sol)?;
    ctx.accounts.user.add_lamports(sol_output)?;
    if fee_recipient_share > 0 {
        ctx.accounts.fee_recipient.add_lamports(fee_recipient_share)?;
    }
    if buyback_share > 0 {
        ctx.accounts.buyback_fee_recipient.add_lamports(buyback_share)?;
    }

    // On a cashback coin, the whole `creator_fee` goes to
    // `user_volume_accumulator` instead of `creator_vault`.
    let is_cashback_coin: bool = ctx.accounts.bonding_curve.is_cashback_coin.into();
    if creator_fee > 0 {
        if is_cashback_coin {
            ctx.accounts.user_volume_accumulator.add_lamports(creator_fee)?;
            let user_volume_accumulator = &mut ctx.accounts.user_volume_accumulator;
            user_volume_accumulator.cashback_earned =
                user_volume_accumulator.cashback_earned.checked_add(creator_fee).ok_or(PumpError::MathOverflow)?;
        } else {
            // The rent-exempt top-up is a real `System::Transfer` CPI from
            // `user`, not sourced from `bonding_curve`'s own debit — see
            // this field's own SAFETY comment for how that was confirmed.
            let current = ctx.accounts.creator_vault.lamports();
            if current < CREATOR_VAULT_RENT_EXEMPT_MINIMUM {
                ctx.accounts.system_program.transfer(
                    SystemTransferAccounts { from: &mut ctx.accounts.user, to: &mut ctx.accounts.creator_vault },
                    CREATOR_VAULT_RENT_EXEMPT_MINIMUM - current,
                )?;
            }
            ctx.accounts.creator_vault.add_lamports(creator_fee)?;
        }
    }

    let bonding_curve = &mut ctx.accounts.bonding_curve;
    bonding_curve.virtual_token_reserves =
        bonding_curve.virtual_token_reserves.checked_add(args.amount).ok_or(PumpError::MathOverflow)?;
    bonding_curve.virtual_quote_reserves =
        bonding_curve.virtual_quote_reserves.checked_sub(gross_sol).ok_or(PumpError::MathOverflow)?;
    bonding_curve.real_token_reserves =
        bonding_curve.real_token_reserves.checked_add(args.amount).ok_or(PumpError::MathOverflow)?;
    bonding_curve.real_quote_reserves =
        bonding_curve.real_quote_reserves.checked_sub(gross_sol).ok_or(PumpError::MathOverflow)?;

    let timestamp = unix_timestamp()?;
    let mint_address = ctx.accounts.mint.address();
    let creator = ctx.accounts.bonding_curve.creator;
    let cashback_fee_basis_points = if is_cashback_coin { fees.creator_fee_bps } else { 0 };
    let bonding_curve = &ctx.accounts.bonding_curve;
    emit!(TradeEvent {
        mint: mint_address,
        sol_amount: gross_sol,
        token_amount: args.amount,
        is_buy: Bool::from(false),
        user: ctx.accounts.user.address(),
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
        ix_name: String::from("sell"),
        mayhem_mode: bonding_curve.is_mayhem_mode,
        cashback_fee_basis_points,
        cashback: if is_cashback_coin { creator_fee } else { 0 },
        buyback_fee_basis_points: ctx.accounts.global.buyback_basis_points,
        buyback_fee: buyback_share,
        shareholders: Vec::new(),
        quote_mint: bonding_curve.quote_mint,
        quote_amount: gross_sol,
        virtual_quote_reserves: bonding_curve.virtual_quote_reserves,
        real_quote_reserves: bonding_curve.real_quote_reserves,
    });

    msg!("Sell successful");
    Ok(())
}
