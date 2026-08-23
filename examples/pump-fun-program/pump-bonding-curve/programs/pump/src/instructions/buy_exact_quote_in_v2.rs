use naclac_lang::prelude::*;
use pump_fees_client::instructions::GetFeesCpiAccounts;
use pump_fees_client::PumpFees;
use crate::components::{BondingCurve, Global, GlobalVolumeAccumulator, UserVolumeAccumulator};
use crate::constants::{
    BONDING_CURVE_SEED, BUYBACK_VAULT_SEED, CREATOR_VAULT_SEED, FEE_CONFIG_SEED, GLOBAL_SEED,
    GLOBAL_VOLUME_ACCUMULATOR_SEED, PUMP_AUTHORITY_SEED, PUMP_FEES_PROGRAM_ID, USER_VOLUME_ACCUMULATOR_SEED,
    WSOL_MINT,
};
use crate::errors::PumpError;
use crate::events::{CompleteEvent, TradeEvent};
use crate::systems::{
    fee_amount_ceil, fee_amount_floor, get_fees_via_cpi, tokens_out_for_net_sol_exact_in, GetFeesParams,
};

// Real `buy_exact_quote_in_v2` shares the exact same 27-account shape as
// `buy_v2` (confirmed via `pump-public-docs/idl/pump.json`), and — per the
// real panic site observed while probing (`programs/pump/src/buy_v2.rs:719`)
// — shares its source file too. Real-behavior-verified for a non-SOL (USDC)
// quote mint via `reference/fee-tier-probe/src/bin/probe44.rs`:
// - `market_cap_lamports` passed to `get_fees` is `0` — matches `buy_v2`'s
//   convention, NOT `buy_exact_sol_in`'s real-nonzero one (that instruction
//   lives in the older, separate `buy.rs`).
// - `trade_size_lamports = spendable_quote_in` (same convention as
//   `buy_exact_sol_in`).
// - `min_tokens_out` must be `> 0` — confirmed real, same `BuyZeroAmount`
//   error as `buy_exact_sol_in` fires for a zero value.
// The quote formula itself is `buy_exact_sol_in`'s real, IDL-documented
// 4-step formula (that instruction's doc comment gives it verbatim), applied
// here to the generic quote asset instead of native SOL specifically.
#[instruction_args]
pub struct BuyExactQuoteInV2Args {
    pub spendable_quote_in: u64,
    pub min_tokens_out: u64,
    pub bonding_curve_bump: u8,
    pub associated_base_bonding_curve_bump: u8,
    pub associated_quote_bonding_curve_bump: u8,
    pub associated_base_user_bump: u8,
    pub associated_quote_user_bump: u8,
    pub creator_vault_bump: u8,
    pub associated_creator_vault_bump: u8,
    pub associated_quote_fee_recipient_bump: u8,
    pub associated_quote_buyback_fee_recipient_bump: u8,
    pub user_volume_accumulator_bump: u8,
    pub associated_user_volume_accumulator_bump: u8,
    pub fee_config_bump: u8,
    pub buyback_index: u8,
    pub buyback_vault_bump: u8,
}

#[derive(Accounts)]
#[instruction(args: BuyExactQuoteInV2Args)]
pub struct BuyExactQuoteInV2 {
    #[account(seeds = [GLOBAL_SEED], bump)]
    pub global: Account<Global>,

    pub base_mint: InterfaceAccount<Mint>,

    #[account(mut)]
    pub quote_mint: InterfaceAccount<Mint>,

    pub base_token_program: Interface<TokenInterface>,
    pub quote_token_program: Interface<TokenInterface>,
    pub associated_token_program: Program<AssociatedToken>,

    /// SAFETY: validated in the handler body against
    /// `{global.fee_recipient} ∪ {global.fee_recipients}` (pool membership,
    /// not a single fixed address — no declarative constraint supports OR);
    /// never deserialized, only a lamport/ATA-transfer destination below.
    #[account(mut)]
    pub fee_recipient: AccountInfo,

    #[account(
        mut,
        init_if_needed,
        payer = user,
        associated_token::mint = quote_mint,
        associated_token::authority = fee_recipient,
        associated_token::bump = args.associated_quote_fee_recipient_bump,
        token::program = quote_token_program,
    )]
    pub associated_quote_fee_recipient: InterfaceAccount<TokenAccount>,

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

    /// SAFETY: real `pump.so` does not auto-create this ATA (confirmed via
    /// probe) — must already exist.
    #[account(
        mut,
        associated_token::mint = quote_mint,
        associated_token::authority = buyback_fee_recipient,
        associated_token::bump = args.associated_quote_buyback_fee_recipient_bump,
        token::program = quote_token_program,
    )]
    pub associated_quote_buyback_fee_recipient: InterfaceAccount<TokenAccount>,

    #[account(
        mut,
        seeds = [BONDING_CURVE_SEED, base_mint.address().as_ref()],
        bump = args.bonding_curve_bump,
    )]
    pub bonding_curve: Account<BondingCurve>,

    #[account(
        mut,
        associated_token::mint = base_mint,
        associated_token::authority = bonding_curve,
        associated_token::bump = args.associated_base_bonding_curve_bump,
        token::program = base_token_program,
    )]
    pub associated_base_bonding_curve: InterfaceAccount<TokenAccount>,

    #[account(
        mut,
        associated_token::mint = quote_mint,
        associated_token::authority = bonding_curve,
        associated_token::bump = args.associated_quote_bonding_curve_bump,
        token::program = quote_token_program,
    )]
    pub associated_quote_bonding_curve: InterfaceAccount<TokenAccount>,

    #[account(mut)]
    pub user: Signer,

    /// SAFETY: must already exist — caller is expected to have created it
    /// beforehand, same as classic `buy`.
    #[account(
        mut,
        associated_token::mint = base_mint,
        associated_token::authority = user,
        associated_token::bump = args.associated_base_user_bump,
        token::program = base_token_program,
    )]
    pub associated_base_user: InterfaceAccount<TokenAccount>,

    /// SAFETY: must already exist — caller is expected to have created it
    /// beforehand. For a SOL-paired coin this is still a real WSOL ATA per
    /// the account list shape, but the actual value transfer uses native
    /// lamports instead (see `buy_v2.rs`'s same treatment).
    #[account(
        mut,
        associated_token::mint = quote_mint,
        associated_token::authority = user,
        associated_token::bump = args.associated_quote_user_bump,
        token::program = quote_token_program,
    )]
    pub associated_quote_user: InterfaceAccount<TokenAccount>,

    /// SAFETY: the `seeds`/`bump` constraint already verifies its address;
    /// it's a lamport-only PDA (no stored data), never `init`'d so still
    /// System-owned — receives a native-SOL rent-exempt top-up regardless of
    /// quote mint (confirmed via probe), never deserialized.
    #[account(
        mut,
        seeds = [CREATOR_VAULT_SEED, bonding_curve.creator.as_ref()],
        bump = args.creator_vault_bump,
    )]
    pub creator_vault: AccountInfo,

    #[account(
        mut,
        init_if_needed,
        payer = user,
        associated_token::mint = quote_mint,
        associated_token::authority = creator_vault,
        associated_token::bump = args.associated_creator_vault_bump,
        token::program = quote_token_program,
    )]
    pub associated_creator_vault: InterfaceAccount<TokenAccount>,

    /// SAFETY: confirmed via probe (`buy_v2`/`sell_v2`) — not read/required
    /// for a coin that hasn't opted into fee-sharing; pure account-shape
    /// parity, same category as `bonding_curve_v2` elsewhere in this program.
    pub sharing_config: AccountInfo,

    /// SAFETY: real `buy_exact_quote_in_v2` never writes this account (same
    /// as classic `buy`/`buy_v2`/`buy_exact_sol_in`) — must already exist,
    /// never `init_if_needed` here.
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

    #[account(
        mut,
        init_if_needed,
        payer = user,
        associated_token::mint = quote_mint,
        associated_token::authority = user_volume_accumulator,
        associated_token::bump = args.associated_user_volume_accumulator_bump,
        token::program = quote_token_program,
    )]
    pub associated_user_volume_accumulator: InterfaceAccount<TokenAccount>,

    /// SAFETY: self-reference, unused beyond seed material for `fee_config`
    /// below — naclac's `emit!` needs no `event_authority`/self-CPI account,
    /// unlike Anchor's own event-emission convention.
    #[account(address = crate::ID)]
    pub program: AccountInfo,

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

    pub system_program: Program<System>,

    /// SAFETY: the `seeds`/`bump` constraint already verifies its address;
    /// it's a lamport-only PDA (no stored data) — only ever used as the
    /// signed-CPI proof-of-origin for `pump_fees::get_fees` below.
    #[account(seeds = [PUMP_AUTHORITY_SEED], bump)]
    pub pump_authority: AccountInfo,
}

#[instruction]
pub fn buy_exact_quote_in_v2(ctx: Context<BuyExactQuoteInV2>, args: BuyExactQuoteInV2Args) -> Result {
    require!(args.spendable_quote_in > 0, PumpError::BuyZeroAmount);
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

    let is_sol_quote = ctx.accounts.quote_mint.address() == WSOL_MINT;
    require!(
        ctx.accounts.quote_mint.address() == ctx.accounts.bonding_curve.quote_mint
            || (ctx.accounts.bonding_curve.quote_mint == Address::default() && is_sol_quote),
        PumpError::QuoteMintMismatch
    );
    require!(
        is_sol_quote || ctx.accounts.global.whitelisted_quote_mints.contains(&ctx.accounts.quote_mint.address()),
        PumpError::QuoteMintNotWhitelisted
    );

    let virtual_token_reserves = ctx.accounts.bonding_curve.virtual_token_reserves;
    let virtual_quote_reserves = ctx.accounts.bonding_curve.virtual_quote_reserves;

    // Confirmed real via probe44: `market_cap_lamports = 0` here (matches
    // `buy_v2`'s convention, not `buy_exact_sol_in`'s).
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
            trade_size_lamports: args.spendable_quote_in,
            is_new_quote_mint: Bool::from(!is_sol_quote),
        },
    )?;

    let has_creator = ctx.accounts.bonding_curve.creator != Address::default();
    let effective_creator_fee_bps = if has_creator { fees.creator_fee_bps } else { 0 };
    let total_fee_bps = fees.protocol_fee_bps.checked_add(effective_creator_fee_bps).ok_or(PumpError::MathOverflow)?;

    // Real IDL-documented 4-step quote formula (same as `buy_exact_sol_in`,
    // confirmed via probe there — applied here to the generic quote asset).
    let mut net_quote = (args.spendable_quote_in as u128)
        .checked_mul(10_000)
        .ok_or(PumpError::MathOverflow)?
        .checked_div(10_000u128.checked_add(total_fee_bps as u128).ok_or(PumpError::MathOverflow)?)
        .ok_or(PumpError::MathOverflow)? as u64;

    let protocol_fee = fee_amount_ceil(net_quote as u128, fees.protocol_fee_bps)? as u64;
    let creator_fee_estimate = fee_amount_ceil(net_quote as u128, effective_creator_fee_bps)? as u64;
    let fees_total = protocol_fee.checked_add(creator_fee_estimate).ok_or(PumpError::MathOverflow)?;

    if let Some(over) = (net_quote.checked_add(fees_total).ok_or(PumpError::MathOverflow)?)
        .checked_sub(args.spendable_quote_in)
        .filter(|&o| o > 0)
    {
        net_quote = net_quote.checked_sub(over).ok_or(PumpError::MathOverflow)?;
    }

    require!(net_quote > 0, PumpError::BuyZeroAmount);
    let tokens_out = tokens_out_for_net_sol_exact_in(
        (net_quote - 1) as u128,
        virtual_token_reserves as u128,
        (virtual_quote_reserves as u128).checked_add((net_quote - 1) as u128).ok_or(PumpError::MathOverflow)?,
    )? as u64;
    require!(tokens_out > 0, PumpError::BuyZeroAmount);
    require!(tokens_out >= args.min_tokens_out, PumpError::SlippageExceeded);
    require!(tokens_out <= ctx.accounts.bonding_curve.real_token_reserves, PumpError::NotEnoughTokensToBuy);

    let protocol_fee = fee_amount_ceil(net_quote as u128, fees.protocol_fee_bps)? as u64;
    let creator_fee = if has_creator { fee_amount_ceil(net_quote as u128, fees.creator_fee_bps)? as u64 } else { 0 };

    let buyback_share = fee_amount_floor(protocol_fee as u128, ctx.accounts.global.buyback_basis_points)? as u64;
    let fee_recipient_share = protocol_fee.checked_sub(buyback_share).ok_or(PumpError::MathOverflow)?;
    let is_cashback_coin: bool = ctx.accounts.bonding_curve.is_cashback_coin.into();
    let quote_decimals = ctx.accounts.quote_mint.decimals();

    if is_sol_quote {
        // Native-lamport path -- same fund flow as classic `buy.rs`/`buy_v2.rs`.
        ctx.accounts.system_program.transfer(
            SystemTransferAccounts { from: &mut ctx.accounts.user, to: &mut ctx.accounts.bonding_curve },
            net_quote,
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
    } else {
        // Real SPL-transfer path, confirmed via probe (same order as
        // `buy_v2`: creator fee / cashback, net amount to the curve,
        // protocol fee share, buyback share).
        if creator_fee > 0 {
            if is_cashback_coin {
                ctx.accounts.quote_token_program.transfer_checked(
                    TransferCheckedAccounts {
                        from: &mut ctx.accounts.associated_quote_user,
                        mint: &ctx.accounts.quote_mint,
                        to: &mut ctx.accounts.associated_user_volume_accumulator,
                        authority: &ctx.accounts.user,
                    },
                    creator_fee,
                    quote_decimals,
                )?;
            } else {
                ctx.accounts.quote_token_program.transfer_checked(
                    TransferCheckedAccounts {
                        from: &mut ctx.accounts.associated_quote_user,
                        mint: &ctx.accounts.quote_mint,
                        to: &mut ctx.accounts.associated_creator_vault,
                        authority: &ctx.accounts.user,
                    },
                    creator_fee,
                    quote_decimals,
                )?;
            }
        }
        ctx.accounts.quote_token_program.transfer_checked(
            TransferCheckedAccounts {
                from: &mut ctx.accounts.associated_quote_user,
                mint: &ctx.accounts.quote_mint,
                to: &mut ctx.accounts.associated_quote_bonding_curve,
                authority: &ctx.accounts.user,
            },
            net_quote,
            quote_decimals,
        )?;
        if fee_recipient_share > 0 {
            ctx.accounts.quote_token_program.transfer_checked(
                TransferCheckedAccounts {
                    from: &mut ctx.accounts.associated_quote_user,
                    mint: &ctx.accounts.quote_mint,
                    to: &mut ctx.accounts.associated_quote_fee_recipient,
                    authority: &ctx.accounts.user,
                },
                fee_recipient_share,
                quote_decimals,
            )?;
        }
        if buyback_share > 0 {
            ctx.accounts.quote_token_program.transfer_checked(
                TransferCheckedAccounts {
                    from: &mut ctx.accounts.associated_quote_user,
                    mint: &ctx.accounts.quote_mint,
                    to: &mut ctx.accounts.associated_quote_buyback_fee_recipient,
                    authority: &ctx.accounts.user,
                },
                buyback_share,
                quote_decimals,
            )?;
        }
    }

    let base_mint_address = ctx.accounts.base_mint.address();
    let signer_seeds: &[&[u8]] = &[BONDING_CURVE_SEED, base_mint_address.as_ref(), &[args.bonding_curve_bump]];
    let signer: &[&[&[u8]]] = &[signer_seeds];
    let base_decimals = ctx.accounts.base_mint.decimals();
    ctx.accounts.base_token_program.transfer_checked_signed(
        TransferCheckedAccounts {
            from: &mut ctx.accounts.associated_base_bonding_curve,
            mint: &ctx.accounts.base_mint,
            to: &mut ctx.accounts.associated_base_user,
            authority: &ctx.accounts.bonding_curve,
        },
        tokens_out,
        base_decimals,
        signer,
    )?;

    let bonding_curve = &mut ctx.accounts.bonding_curve;
    bonding_curve.virtual_token_reserves =
        bonding_curve.virtual_token_reserves.checked_sub(tokens_out).ok_or(PumpError::MathOverflow)?;
    bonding_curve.virtual_quote_reserves =
        bonding_curve.virtual_quote_reserves.checked_add(net_quote).ok_or(PumpError::MathOverflow)?;
    bonding_curve.real_token_reserves =
        bonding_curve.real_token_reserves.checked_sub(tokens_out).ok_or(PumpError::MathOverflow)?;
    bonding_curve.real_quote_reserves =
        bonding_curve.real_quote_reserves.checked_add(net_quote).ok_or(PumpError::MathOverflow)?;
    if bonding_curve.real_token_reserves == 0 {
        bonding_curve.complete = true.into();
    }

    let timestamp = unix_timestamp()?;
    let user_address = ctx.accounts.user.address();
    let creator = ctx.accounts.bonding_curve.creator;
    let complete: bool = ctx.accounts.bonding_curve.complete.into();
    let cashback_fee_basis_points = if is_cashback_coin { fees.creator_fee_bps } else { 0 };
    let quote_mint_address = ctx.accounts.quote_mint.address();
    let bonding_curve = &ctx.accounts.bonding_curve;
    emit!(TradeEvent {
        mint: base_mint_address,
        sol_amount: net_quote,
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
        ix_name: String::from("buy_exact_quote_in_v2"),
        mayhem_mode: bonding_curve.is_mayhem_mode,
        cashback_fee_basis_points,
        cashback: if is_cashback_coin { creator_fee } else { 0 },
        buyback_fee_basis_points: ctx.accounts.global.buyback_basis_points,
        buyback_fee: buyback_share,
        shareholders: Vec::new(),
        quote_mint: quote_mint_address,
        quote_amount: net_quote,
        virtual_quote_reserves: bonding_curve.virtual_quote_reserves,
        real_quote_reserves: bonding_curve.real_quote_reserves,
    });

    if complete {
        emit!(CompleteEvent {
            user: user_address,
            mint: base_mint_address,
            bonding_curve: ctx.accounts.bonding_curve.address(),
            timestamp,
            quote_mint: quote_mint_address,
        });
    }

    msg!("Buy successful");
    Ok(())
}
