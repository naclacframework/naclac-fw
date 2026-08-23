use naclac_lang::prelude::*;
use pump_amm_client::instructions::{
    CreatePoolCpi, CreatePoolCpiAccounts, InitBoostCpi, InitBoostCpiAccounts,
};
use pump_amm_client::types::CreatePoolArgsCpi as PumpAmmCreatePoolArgs;
use pump_amm_client::PumpAmm;
use crate::components::{BondingCurve, Global};
use crate::constants::{
    ASSOCIATED_TOKEN_PROGRAM_ID, BONDING_CURVE_SEED, BOOST_VAULT_SEED, GLOBAL_CONFIG_SEED,
    GLOBAL_SEED, POOL_AUTHORITY_SEED, POOL_LP_MINT_SEED, POOL_SEED, PUMP_AMM_PROGRAM_ID, WSOL_MINT,
};
use crate::errors::PumpError;
use crate::events::CompletePumpAmmMigrationEvent;

// Real `migrate_v2` has `args: []` — same on-chain-search-ban divergence as
// `migrate.rs`'s own `MigrateArgs`, plus two more bumps for the boost-vault
// PDAs `migrate_v2` additionally CPIs into via `init_boost` (confirmed real
// `remaining_accounts = [boost_vault_authority, boost_vault]`, see
// `docs/plan/bonding-curve-05-batch1-v2-instructions.md`).
#[instruction_args]
pub struct MigrateV2Args {
    pub bonding_curve_bump: u8,
    pub associated_base_bonding_curve_bump: u8,
    pub associated_quote_bonding_curve_bump: u8,
    pub pool_authority_bump: u8,
    pub pool_authority_mint_account_bump: u8,
    pub pool_authority_quote_account_bump: u8,
    pub amm_global_config_bump: u8,
    pub pool_bump: u8,
    pub lp_mint_bump: u8,
    pub user_pool_token_account_bump: u8,
    pub pool_base_token_account_bump: u8,
    pub pool_quote_token_account_bump: u8,
    pub boost_vault_authority_bump: u8,
    pub boost_vault_bump: u8,
}

// Confirmed empirically against real, deployed `pump.so`/`pump_amm.so`
// (`reference/fee-tier-probe/src/bin/probe45.rs`/`probe46.rs`, plus 5
// independent real mainnet transactions — `docs/plan/bonding-curve-05-batch1-v2-instructions.md`
// and `docs/plan/amm-03-boost-mechanism.md`):
// - Real fund-flow difference from classic `migrate`: `pool_authority`'s
//   account-creation rent (exactly `global.pool_migration_fee`) is funded
//   directly by `user` via a real `System::Transfer`, NOT sourced from
//   `bonding_curve`'s own lamport balance the way `migrate.rs` does — the
//   quote-generalized path has nowhere else to source it from, since
//   `real_quote_reserves` for a non-SOL curve is real SPL token balance, not
//   lamports.
// - `quote_amount_in` passed to the nested `create_pool` CPI is the FULL
//   `associated_quote_bonding_curve` balance — no fee/split taken here.
// - After `create_pool` succeeds, `migrate_v2` unconditionally CPIs into
//   `pump_amm::init_boost` with `remaining_accounts = [boost_vault_authority,
//   boost_vault]` — `init_boost` itself computes and moves the boost split
//   (see `pump_amm::instructions::init_boost`'s own doc comment), `migrate_v2`
//   does not compute anything boost-related itself.
// - For a SOL-paired curve, `buy_v2`/`sell_v2` never fund
//   `associated_quote_bonding_curve` with real WSOL (confirmed via
//   `reference/fee-tier-probe/src/bin/probe51.rs` — see `buy_v2.rs`'s own
//   module comment) — the real quote value lives entirely in
//   `bonding_curve`'s native lamport balance instead, tracked exactly by
//   `real_quote_reserves`. `migrate_v2` wraps that balance into real WSOL
//   (`sub_lamports`/`add_lamports`/`sync_native` on
//   `associated_quote_bonding_curve`) before the existing SPL transfer,
//   mirroring `migrate.rs`'s own native-SOL-to-WSOL wrap for the same
//   reason.
#[derive(Accounts)]
#[instruction(args: MigrateV2Args)]
pub struct MigrateV2 {
    #[account(seeds = [GLOBAL_SEED], bump)]
    pub global: Account<Global>,

    /// SAFETY: validated in the handler body against `global.withdraw_authority`
    /// (a single dynamic field, not a static address) — no declarative
    /// constraint supports comparing against another account's field value.
    #[account(mut)]
    pub withdraw_authority: AccountInfo,

    pub base_mint: InterfaceAccount<Mint>,

    #[account(mut)]
    pub quote_mint: InterfaceAccount<Mint>,

    /// SAFETY: `quote_mint` validated in the handler body against
    /// `bonding_curve.quote_mint`, not via a declarative relational
    /// constraint — a SOL-paired curve stores `Address::default()` there
    /// (never literally `WSOL_MINT`), and naclac's `#[account(field =
    /// target)]` syntax compiles to a single hard equality with no OR
    /// support, so that default-address exception can't be expressed
    /// declaratively (same reasoning already applied in `buy_v2`/`sell_v2`).
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

    pub system_program: Program<System>,
    pub base_token_program: Interface<TokenInterface>,
    pub quote_token_program: Interface<TokenInterface>,
    pub token_2022_program: Program<Token2022>,
    pub associated_token_program: Program<AssociatedToken>,
    pub pump_amm: Program<PumpAmm>,

    /// SAFETY: address pinned via `address = RENT_SYSVAR_ID`; genuinely part
    /// of the real, confirmed 27-account `migrate_v2` list (already
    /// established via a real mainnet transaction — see module comment) but
    /// not wired into this implementation until now. Needed for real: the
    /// SOL-quote wrap below passes it to `sync_native_with_extra_accounts`,
    /// whose second account slot the real classic-Token `SyncNative`
    /// processor specifically requires to be the Rent sysvar (same as
    /// `migrate.rs`'s own identical call).
    #[account(address = RENT_SYSVAR_ID)]
    pub rent: AccountInfo,

    /// SAFETY: lamport-only PDA (no stored data) — real payer for every
    /// account created during migration; signs the nested `create_pool`/
    /// `init_boost` CPIs as `creator` via `invoke_signed`.
    #[account(
        mut,
        seeds = [POOL_AUTHORITY_SEED, base_mint.address().as_ref()],
        bump = args.pool_authority_bump,
    )]
    pub pool_authority: AccountInfo,

    /// SAFETY: doesn't exist yet — created by the nested `create_pool` CPI
    /// below, address verified via seeds only.
    #[account(
        mut,
        seeds = [POOL_SEED, &0u16.to_le_bytes(), pool_authority.address().as_ref(), base_mint.address().as_ref(), quote_mint.address().as_ref()],
        seeds::program = PUMP_AMM_PROGRAM_ID,
        bump = args.pool_bump,
    )]
    pub pool: AccountInfo,

    /// SAFETY: created directly by this instruction's own body (a manual
    /// `create_idempotent_with_extra_accounts_signed` call, same reasoning as
    /// `migrate.rs`'s own account of the same name). Address verified via
    /// `seeds`/`seeds::program`/`bump` regardless.
    #[account(
        mut,
        seeds = [pool_authority.address().as_ref(), base_token_program.address().as_ref(), base_mint.address().as_ref()],
        seeds::program = ASSOCIATED_TOKEN_PROGRAM_ID,
        bump = args.pool_authority_mint_account_bump,
    )]
    pub pool_authority_mint_account: AccountInfo,

    /// SAFETY: same as `pool_authority_mint_account` above.
    #[account(
        mut,
        seeds = [pool_authority.address().as_ref(), quote_token_program.address().as_ref(), quote_mint.address().as_ref()],
        seeds::program = ASSOCIATED_TOKEN_PROGRAM_ID,
        bump = args.pool_authority_quote_account_bump,
    )]
    pub pool_authority_quote_account: AccountInfo,

    /// SAFETY: address fully verified via `seeds`/`seeds::program`/`bump`;
    /// passed straight into the nested `create_pool`/`init_boost` CPIs below.
    #[account(
        mut,
        seeds = [GLOBAL_CONFIG_SEED],
        seeds::program = PUMP_AMM_PROGRAM_ID,
        bump = args.amm_global_config_bump,
    )]
    pub amm_global_config: AccountInfo,

    /// SAFETY: doesn't exist yet — created by the nested `create_pool` CPI.
    #[account(
        mut,
        seeds = [POOL_LP_MINT_SEED, pool.address().as_ref()],
        seeds::program = PUMP_AMM_PROGRAM_ID,
        bump = args.lp_mint_bump,
    )]
    pub lp_mint: AccountInfo,

    /// SAFETY: doesn't exist yet — created by the nested `create_pool` CPI;
    /// `pool_authority`'s own LP-token ATA, burned and closed below once
    /// minted so no one ends up holding the migrated pool's liquidity.
    #[account(
        mut,
        seeds = [pool_authority.address().as_ref(), token_2022_program.address().as_ref(), lp_mint.address().as_ref()],
        seeds::program = ASSOCIATED_TOKEN_PROGRAM_ID,
        bump = args.user_pool_token_account_bump,
    )]
    pub user_pool_token_account: AccountInfo,

    /// SAFETY: created directly by this instruction's own body — see
    /// `pool_authority_mint_account` above for why.
    #[account(
        mut,
        seeds = [pool.address().as_ref(), base_token_program.address().as_ref(), base_mint.address().as_ref()],
        seeds::program = ASSOCIATED_TOKEN_PROGRAM_ID,
        bump = args.pool_base_token_account_bump,
    )]
    pub pool_base_token_account: AccountInfo,

    /// SAFETY: same as `pool_base_token_account` above.
    #[account(
        mut,
        seeds = [pool.address().as_ref(), quote_token_program.address().as_ref(), quote_mint.address().as_ref()],
        seeds::program = ASSOCIATED_TOKEN_PROGRAM_ID,
        bump = args.pool_quote_token_account_bump,
    )]
    pub pool_quote_token_account: AccountInfo,

    /// SAFETY: `seeds`/`bump` already verifies its address; a bare
    /// signing/seed PDA with no stored data — real `remaining_accounts[0]`
    /// on `migrate_v2`, confirmed via a real mainnet transaction (see
    /// module doc comment above).
    #[account(
        seeds = [BOOST_VAULT_SEED, pool.address().as_ref()],
        seeds::program = PUMP_AMM_PROGRAM_ID,
        bump = args.boost_vault_authority_bump,
    )]
    pub boost_vault_authority: AccountInfo,

    /// SAFETY: doesn't exist yet — created by the nested `init_boost` CPI.
    /// Real `remaining_accounts[1]` on `migrate_v2`.
    #[account(
        mut,
        seeds = [boost_vault_authority.address().as_ref(), quote_token_program.address().as_ref(), quote_mint.address().as_ref()],
        seeds::program = ASSOCIATED_TOKEN_PROGRAM_ID,
        bump = args.boost_vault_bump,
    )]
    pub boost_vault: AccountInfo,
}

#[instruction]
pub fn migrate_v2(ctx: Context<MigrateV2>, args: MigrateV2Args) -> Result {
    let is_complete: bool = ctx.accounts.bonding_curve.complete.into();
    require!(is_complete, PumpError::NotComplete);

    // Real, confirmed via `reference/fee-tier-probe/src/bin/probe56.rs`
    // through `probe63.rs` (against classic `migrate`; independently
    // corroborated for `migrate_v2` by real mainnet transactions logging
    // the identical "Bonding curve already migrated" line — see
    // `docs/plan/bonding-curve-05-batch1-v2-instructions.md`): a curve
    // whose four reserve fields are all already zero is treated as already
    // migrated and returns successfully without running any CPI below.
    if ctx.accounts.bonding_curve.virtual_token_reserves == 0
        && ctx.accounts.bonding_curve.virtual_quote_reserves == 0
        && ctx.accounts.bonding_curve.real_token_reserves == 0
        && ctx.accounts.bonding_curve.real_quote_reserves == 0
    {
        msg!("Bonding curve already migrated");
        return Ok(());
    }

    let enable_migrate: bool = ctx.accounts.global.enable_migrate.into();
    require!(enable_migrate, PumpError::MigrateDisabled);
    require!(
        ctx.accounts.withdraw_authority.address() == ctx.accounts.global.withdraw_authority,
        PumpError::InvalidWithdrawAuthority
    );
    let is_sol_quote = ctx.accounts.quote_mint.address() == WSOL_MINT;
    require!(
        ctx.accounts.quote_mint.address() == ctx.accounts.bonding_curve.quote_mint
            || (ctx.accounts.bonding_curve.quote_mint == Address::default() && is_sol_quote),
        PumpError::QuoteMintMismatch
    );

    let pool_migration_fee = ctx.accounts.global.pool_migration_fee;
    let base_token_amount = ctx.accounts.associated_base_bonding_curve.amount();
    // For a SOL-paired curve, `buy_v2`/`sell_v2` never fund
    // `associated_quote_bonding_curve` — the real quote value lives in
    // `bonding_curve`'s own native lamport balance, tracked exactly by
    // `real_quote_reserves` (confirmed via `reference/fee-tier-probe/src/bin/probe51.rs`;
    // see `buy_v2.rs`'s module comment). Wrapped into real WSOL below before
    // the existing SPL transfer, mirroring `migrate.rs`'s own
    // native-SOL-to-WSOL wrap for the same reason.
    let quote_token_amount = if is_sol_quote {
        ctx.accounts.bonding_curve.real_quote_reserves
    } else {
        ctx.accounts.associated_quote_bonding_curve.amount()
    };

    let base_mint_address = ctx.accounts.base_mint.address();
    let quote_mint_address = ctx.accounts.quote_mint.address();
    let quote_decimals = ctx.accounts.quote_mint.decimals();
    let base_decimals = ctx.accounts.base_mint.decimals();
    let bonding_curve_signer_seeds: &[&[u8]] =
        &[BONDING_CURVE_SEED, base_mint_address.as_ref(), &[args.bonding_curve_bump]];
    let bonding_curve_signer: &[&[&[u8]]] = &[bonding_curve_signer_seeds];

    let pool_authority_signer_seeds: &[&[u8]] =
        &[POOL_AUTHORITY_SEED, base_mint_address.as_ref(), &[args.pool_authority_bump]];
    let pool_authority_signer: &[&[&[u8]]] = &[pool_authority_signer_seeds];

    // `pool_authority`'s rent is funded directly by `user` — a real, tracked
    // `SystemProgram::transfer`, not a raw lamport write (confirmed real
    // fund-flow difference from `migrate.rs`, see module doc comment above).
    ctx.accounts.system_program.transfer(
        SystemTransferAccounts { from: &mut ctx.accounts.user, to: &mut ctx.accounts.pool_authority },
        pool_migration_fee,
    )?;

    let pool_authority_copy = ctx.accounts.pool_authority;

    naclac_lang::associated_token::create_idempotent_signed(
        ctx.accounts.associated_token_program.to_cpi_handle(),
        naclac_lang::associated_token::AtaCpiAccounts {
            payer: ctx.accounts.pool_authority.to_cpi_handle_mut(),
            associated_token: ctx.accounts.pool_base_token_account.to_cpi_handle_mut(),
            authority: ctx.accounts.pool.to_cpi_handle(),
            mint: ctx.accounts.base_mint.to_cpi_handle(),
            system_program: ctx.accounts.system_program.to_cpi_handle(),
            token_program: ctx.accounts.base_token_program.to_cpi_handle(),
        },
        pool_authority_signer,
    )?;
    naclac_lang::associated_token::create_idempotent_signed(
        ctx.accounts.associated_token_program.to_cpi_handle(),
        naclac_lang::associated_token::AtaCpiAccounts {
            payer: ctx.accounts.pool_authority.to_cpi_handle_mut(),
            associated_token: ctx.accounts.pool_quote_token_account.to_cpi_handle_mut(),
            authority: ctx.accounts.pool.to_cpi_handle(),
            mint: ctx.accounts.quote_mint.to_cpi_handle(),
            system_program: ctx.accounts.system_program.to_cpi_handle(),
            token_program: ctx.accounts.quote_token_program.to_cpi_handle(),
        },
        pool_authority_signer,
    )?;
    naclac_lang::associated_token::create_idempotent_signed(
        ctx.accounts.associated_token_program.to_cpi_handle(),
        naclac_lang::associated_token::AtaCpiAccounts {
            payer: ctx.accounts.pool_authority.to_cpi_handle_mut(),
            associated_token: ctx.accounts.pool_authority_mint_account.to_cpi_handle_mut(),
            authority: pool_authority_copy.to_cpi_handle(),
            mint: ctx.accounts.base_mint.to_cpi_handle(),
            system_program: ctx.accounts.system_program.to_cpi_handle(),
            token_program: ctx.accounts.base_token_program.to_cpi_handle(),
        },
        pool_authority_signer,
    )?;
    naclac_lang::associated_token::create_idempotent_signed(
        ctx.accounts.associated_token_program.to_cpi_handle(),
        naclac_lang::associated_token::AtaCpiAccounts {
            payer: ctx.accounts.pool_authority.to_cpi_handle_mut(),
            associated_token: ctx.accounts.pool_authority_quote_account.to_cpi_handle_mut(),
            authority: pool_authority_copy.to_cpi_handle(),
            mint: ctx.accounts.quote_mint.to_cpi_handle(),
            system_program: ctx.accounts.system_program.to_cpi_handle(),
            token_program: ctx.accounts.quote_token_program.to_cpi_handle(),
        },
        pool_authority_signer,
    )?;

    // Full balances move untouched — no fee taken on either side during
    // migration (confirmed via probe, same as `migrate.rs`).
    ctx.accounts.base_token_program.transfer_checked_signed(
        TransferCheckedAccounts {
            from: &mut ctx.accounts.associated_base_bonding_curve,
            mint: &ctx.accounts.base_mint,
            to: &mut ctx.accounts.pool_authority_mint_account,
            authority: &ctx.accounts.bonding_curve,
        },
        base_token_amount,
        base_decimals,
        bonding_curve_signer,
    )?;

    if is_sol_quote {
        // Raw lamport writes, not a `System::Transfer` CPI — `bonding_curve`
        // is owned by this program, not the System Program, so it can't
        // sign a `System::Transfer` as source at all. This is exactly
        // `migrate.rs`'s own real, probe-confirmed pattern (see its module
        // comment): the runtime only folds a raw write into its tracked
        // bookkeeping for accounts present in the very next CPI's own
        // account list, so `bonding_curve` must ride along as an extra,
        // unread account on the `sync_native` CPI that follows.
        ctx.accounts.bonding_curve.sub_lamports(quote_token_amount)?;
        ctx.accounts.associated_quote_bonding_curve.add_lamports(quote_token_amount)?;
        let bonding_curve_extra: [CpiHandle<'_>; 1] = [ctx.accounts.bonding_curve.info.to_cpi_handle()];
        naclac_lang::token::sync_native_with_extra_accounts(
            ctx.accounts.quote_token_program.to_cpi_handle(),
            ctx.accounts.associated_quote_bonding_curve.to_cpi_handle_mut(),
            ctx.accounts.rent.to_cpi_handle(),
            &bonding_curve_extra,
        )?;
    }
    ctx.accounts.quote_token_program.transfer_checked_signed(
        TransferCheckedAccounts {
            from: &mut ctx.accounts.associated_quote_bonding_curve,
            mint: &ctx.accounts.quote_mint,
            to: &mut ctx.accounts.pool_authority_quote_account,
            authority: &ctx.accounts.bonding_curve,
        },
        quote_token_amount,
        quote_decimals,
        bonding_curve_signer,
    )?;

    ctx.accounts.pump_amm.create_pool_signed(
        CreatePoolCpiAccounts {
            creator: ctx.accounts.pool_authority.to_cpi_handle_mut(),
            base_mint: ctx.accounts.base_mint.to_cpi_handle(),
            quote_mint: ctx.accounts.quote_mint.to_cpi_handle(),
            global_config: ctx.accounts.amm_global_config.to_cpi_handle(),
            pool: ctx.accounts.pool.to_cpi_handle_mut(),
            lp_mint: ctx.accounts.lp_mint.to_cpi_handle_mut(),
            user_base_token_account: ctx.accounts.pool_authority_mint_account.to_cpi_handle_mut(),
            user_quote_token_account: ctx.accounts.pool_authority_quote_account.to_cpi_handle_mut(),
            user_pool_token_account: ctx.accounts.user_pool_token_account.to_cpi_handle_mut(),
            pool_base_token_account: ctx.accounts.pool_base_token_account.to_cpi_handle_mut(),
            pool_quote_token_account: ctx.accounts.pool_quote_token_account.to_cpi_handle_mut(),
            system_program: ctx.accounts.system_program.to_cpi_handle(),
            token_2022_program: ctx.accounts.token_2022_program.to_cpi_handle(),
            base_token_program: ctx.accounts.base_token_program.to_cpi_handle(),
            quote_token_program: ctx.accounts.quote_token_program.to_cpi_handle(),
            associated_token_program: ctx.accounts.associated_token_program.to_cpi_handle(),
        },
        PumpAmmCreatePoolArgs {
            index: 0,
            base_amount_in: base_token_amount,
            quote_amount_in: quote_token_amount,
            coin_creator: ctx.accounts.bonding_curve.creator,
            is_mayhem_mode: ctx.accounts.bonding_curve.is_mayhem_mode,
            is_cashback_coin: ctx.accounts.bonding_curve.is_cashback_coin,
            pool_bump: args.pool_bump,
            lp_mint_bump: args.lp_mint_bump,
            user_base_token_account_bump: args.pool_authority_mint_account_bump,
            user_quote_token_account_bump: args.pool_authority_quote_account_bump,
            user_pool_token_account_bump: args.user_pool_token_account_bump,
            pool_base_token_account_bump: args.pool_base_token_account_bump,
            pool_quote_token_account_bump: args.pool_quote_token_account_bump,
        },
        pool_authority_signer,
    )?;

    // Confirmed unconditional (`amm-03-boost-mechanism.md`): `migrate_v2`
    // always CPIs into `init_boost` after `create_pool` succeeds.
    // `init_boost` itself computes and moves the boost split, reading
    // `pool_quote_token_account`/`pool_base_token_account`'s balances as
    // they stand right after `create_pool` deposited them.
    ctx.accounts.pump_amm.init_boost_signed(
        InitBoostCpiAccounts {
            bonding_curve: ctx.accounts.bonding_curve.info.to_cpi_handle(),
            pool: ctx.accounts.pool.to_cpi_handle_mut(),
            global_config: ctx.accounts.amm_global_config.to_cpi_handle(),
            creator: ctx.accounts.pool_authority.to_cpi_handle_mut(),
            base_mint: ctx.accounts.base_mint.to_cpi_handle(),
            quote_mint: ctx.accounts.quote_mint.to_cpi_handle(),
            pool_base_token_account: ctx.accounts.pool_base_token_account.to_cpi_handle(),
            pool_quote_token_account: ctx.accounts.pool_quote_token_account.to_cpi_handle_mut(),
            boost_vault_authority: ctx.accounts.boost_vault_authority.to_cpi_handle(),
            boost_vault: ctx.accounts.boost_vault.to_cpi_handle_mut(),
            quote_token_program: ctx.accounts.quote_token_program.to_cpi_handle(),
            system_program: ctx.accounts.system_program.to_cpi_handle(),
            associated_token_program: ctx.accounts.associated_token_program.to_cpi_handle(),
        },
        args.boost_vault_authority_bump,
        args.boost_vault_bump,
        pool_authority_signer,
    )?;

    // No one ends up holding the migrated pool's initial liquidity —
    // confirmed via probe (`Burn` then `CloseAccount` on the real program).
    let lp_minted = InterfaceAccount::<TokenAccount>::try_from(&ctx.accounts.user_pool_token_account, 0)?.amount();
    ctx.accounts.token_2022_program.burn_signed(
        BurnAccounts {
            mint: &mut ctx.accounts.lp_mint,
            from: &mut ctx.accounts.user_pool_token_account,
            authority: &ctx.accounts.pool_authority,
        },
        lp_minted,
        pool_authority_signer,
    )?;
    let pool_authority_for_close = ctx.accounts.pool_authority;
    ctx.accounts.token_2022_program.close_account_signed(
        CloseAccountAccounts {
            account: &mut ctx.accounts.user_pool_token_account,
            destination: &mut ctx.accounts.pool_authority,
            authority: &pool_authority_for_close,
        },
        pool_authority_signer,
    )?;

    // Any lamports `pool_authority` still holds beyond what account creation
    // actually consumed sweep to `withdraw_authority` (same pattern as
    // `migrate.rs`).
    let pool_authority_leftover = ctx.accounts.pool_authority.lamports();
    if pool_authority_leftover > 0 {
        ctx.accounts.system_program.transfer_signed(
            SystemTransferAccounts {
                from: &mut ctx.accounts.pool_authority,
                to: &mut ctx.accounts.withdraw_authority,
            },
            pool_authority_leftover,
            pool_authority_signer,
        )?;
    }

    let bonding_curve = &mut ctx.accounts.bonding_curve;
    bonding_curve.virtual_token_reserves = 0;
    bonding_curve.virtual_quote_reserves = 0;
    bonding_curve.real_token_reserves = 0;
    bonding_curve.real_quote_reserves = 0;

    let timestamp = unix_timestamp()?;
    emit!(CompletePumpAmmMigrationEvent {
        timestamp,
        mint_amount: base_token_amount,
        sol_amount: quote_token_amount,
        pool_migration_fee,
        user: ctx.accounts.user.address(),
        mint: base_mint_address,
        bonding_curve: ctx.accounts.bonding_curve.address(),
        pool: ctx.accounts.pool.address(),
        quote_mint: quote_mint_address,
    });

    msg!("Bonding curve successfully migrated");
    Ok(())
}
