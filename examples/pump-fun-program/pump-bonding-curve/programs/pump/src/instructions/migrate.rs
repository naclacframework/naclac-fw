use naclac_lang::prelude::*;
use pump_amm_client::instructions::{CreatePoolCpi, CreatePoolCpiAccounts};
use pump_amm_client::types::CreatePoolArgsCpi as PumpAmmCreatePoolArgs;
use pump_amm_client::PumpAmm;
use crate::components::{BondingCurve, Global};
use crate::constants::{
    ASSOCIATED_TOKEN_PROGRAM_ID, BONDING_CURVE_SEED, GLOBAL_CONFIG_SEED, GLOBAL_SEED,
    POOL_AUTHORITY_SEED, POOL_LP_MINT_SEED, POOL_SEED, PUMP_AMM_PROGRAM_ID, WSOL_MINT,
};
use crate::errors::PumpError;
use crate::events::CompletePumpAmmMigrationEvent;

// Real `migrate` has `args: []` — every PDA bump gets found via an on-chain
// search loop, which naclac forbids for non-compile-time-literal seeds.
// Every dynamic PDA below therefore takes an explicit caller-supplied bump,
// same pattern as `pump_amm::create_pool`'s own `CreatePoolArgs`.
#[instruction_args]
pub struct MigrateArgs {
    pub bonding_curve_bump: u8,
    pub associated_bonding_curve_bump: u8,
    pub pool_authority_bump: u8,
    pub pool_authority_mint_account_bump: u8,
    pub pool_authority_wsol_account_bump: u8,
    pub amm_global_config_bump: u8,
    pub pool_bump: u8,
    pub lp_mint_bump: u8,
    pub user_pool_token_account_bump: u8,
    pub pool_base_token_account_bump: u8,
    pub pool_quote_token_account_bump: u8,
}

// Confirmed empirically (`reference/fee-tier-probe/src/bin/probe31-36.rs`
// against real, deployed `pump.so`/`pump_amm.so`) before writing this:
// `migrate` moves `associated_bonding_curve`'s full token balance — not the
// `real_token_reserves` counter, which only tracks the tradeable-via-curve
// portion and legitimately reaches zero at graduation (`probe35`) — into the
// new pool's base side (`probe36`: with `real_token_reserves = 0`, the real
// program still transferred the account's full nonzero balance and used it
// as `create_pool`'s `base_amount_in`). Splits `real_quote_reserves` into
// `global.pool_migration_fee` (funds `pool_authority`'s rent for every
// account it pays to create during migration, any unspent remainder swept
// to `withdraw_authority` at the end) and the remainder (wrapped to WSOL,
// deposited as the pool's quote side); CPIs into `pump_amm::create_pool`
// signed by `pool_authority`; then burns and closes the resulting LP-token
// account so no one holds the migrated pool's initial liquidity.
//
// `pool_authority` — not `user` — pays for every account created here,
// funded entirely from `pool_migration_fee` (confirmed via `probe33`: the
// migration succeeds even with `pool_authority` starting at zero lamports,
// as long as `pool_migration_fee` is nonzero). `pool_base_token_account`/
// `pool_quote_token_account` are created directly by `migrate` itself (not
// left to `pump_amm::create_pool`'s own internal creation, confirmed via
// `probe31`'s decoded inner instructions showing both already existing by
// the time the nested `CreatePool` CPI starts) — `pump_amm::create_pool`
// must therefore tolerate them already existing (`init_if_needed`, not
// strict `init`) when invoked this way.
#[derive(Accounts)]
#[instruction(args: MigrateArgs)]
pub struct Migrate {
    #[account(seeds = [GLOBAL_SEED], bump)]
    pub global: Account<Global>,

    /// SAFETY: validated in the handler body against `global.withdraw_authority`
    /// (a single dynamic field, not a static address) — no declarative
    /// constraint supports comparing against another account's field value.
    #[account(mut)]
    pub withdraw_authority: AccountInfo,

    pub mint: InterfaceAccount<Mint>,

    #[account(
        mut,
        seeds = [BONDING_CURVE_SEED, mint.address().as_ref()],
        bump = args.bonding_curve_bump,
    )]
    pub bonding_curve: Account<BondingCurve>,

    /// SAFETY: address pinned via `address = WSOL_MINT`; only used as CPI/seed
    /// material below, never deserialized.
    #[account(address = WSOL_MINT)]
    pub wsol_mint: AccountInfo,

    #[account(mut)]
    pub user: Signer,

    pub system_program: Program<System>,
    pub token_program: Interface<TokenInterface>,
    pub token_2022_program: Program<Token2022>,
    pub associated_token_program: Program<AssociatedToken>,
    pub pump_amm: Program<PumpAmm>,

    /// SAFETY: address pinned via `address = RENT_SYSVAR_ID`; passed to the
    /// manual `sync_native_with_extra_accounts` call below, whose second
    /// account slot the real classic-Token `SyncNative` processor
    /// specifically requires to be the Rent sysvar (confirmed empirically —
    /// see that function's own doc comment in `naclac-token`).
    #[account(address = RENT_SYSVAR_ID)]
    pub rent: AccountInfo,

    #[account(
        mut,
        associated_token::mint = mint,
        associated_token::authority = bonding_curve,
        associated_token::bump = args.associated_bonding_curve_bump,
        token::program = token_program,
    )]
    pub associated_bonding_curve: InterfaceAccount<TokenAccount>,

    /// SAFETY: lamport-only PDA (no stored data) — real payer for every
    /// account created during migration (see module comment); signs the
    /// nested `create_pool` CPI as `creator` via `invoke_signed`.
    #[account(
        mut,
        seeds = [POOL_AUTHORITY_SEED, mint.address().as_ref()],
        bump = args.pool_authority_bump,
    )]
    pub pool_authority: AccountInfo,

    /// SAFETY: doesn't exist yet — created by the nested `create_pool` CPI
    /// below, address verified via seeds only.
    #[account(
        mut,
        seeds = [POOL_SEED, &0u16.to_le_bytes(), pool_authority.address().as_ref(), mint.address().as_ref(), WSOL_MINT.as_ref()],
        seeds::program = PUMP_AMM_PROGRAM_ID,
        bump = args.pool_bump,
    )]
    pub pool: AccountInfo,

    /// SAFETY: created directly by this instruction's own body (a manual
    /// `create_idempotent_with_extra_accounts_signed` call, not the
    /// declarative `init`/`associated_token::` sugar — `pool_authority`, its
    /// payer, is itself a raw-funded PDA whose lamport credit must be
    /// reconciled alongside `bonding_curve`'s matching debit in the very
    /// same CPI, which the sugar's own account-loading-time execution order
    /// can't accommodate). Address verified via `seeds`/`seeds::program`/
    /// `bump` below regardless, same as `pool_base_token_account`.
    #[account(
        mut,
        seeds = [pool_authority.address().as_ref(), token_program.address().as_ref(), mint.address().as_ref()],
        seeds::program = ASSOCIATED_TOKEN_PROGRAM_ID,
        bump = args.pool_authority_mint_account_bump,
    )]
    pub pool_authority_mint_account: AccountInfo,

    /// SAFETY: same as `pool_authority_mint_account` above.
    #[account(
        mut,
        seeds = [pool_authority.address().as_ref(), token_program.address().as_ref(), wsol_mint.address().as_ref()],
        seeds::program = ASSOCIATED_TOKEN_PROGRAM_ID,
        bump = args.pool_authority_wsol_account_bump,
    )]
    pub pool_authority_wsol_account: AccountInfo,

    /// SAFETY: address fully verified via `seeds`/`seeds::program`/`bump`;
    /// passed straight into the nested `create_pool` CPI below, whose own
    /// program-side validation covers its contents — never deserialized here.
    #[account(
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
    /// this is `pool_authority`'s own LP-token ATA, burned and closed below
    /// once minted so no one ends up holding the migrated pool's liquidity.
    #[account(
        mut,
        seeds = [pool_authority.address().as_ref(), token_2022_program.address().as_ref(), lp_mint.address().as_ref()],
        seeds::program = ASSOCIATED_TOKEN_PROGRAM_ID,
        bump = args.user_pool_token_account_bump,
    )]
    pub user_pool_token_account: AccountInfo,

    /// SAFETY: created directly by this instruction's own body — see
    /// `pool_authority_mint_account` above for why. Owned by `pool` (not
    /// `pool_authority`), so its own creation CPI needs no aliasing tricks:
    /// `payer = pool_authority`, `authority = pool` are different accounts.
    #[account(
        mut,
        seeds = [pool.address().as_ref(), token_program.address().as_ref(), mint.address().as_ref()],
        seeds::program = ASSOCIATED_TOKEN_PROGRAM_ID,
        bump = args.pool_base_token_account_bump,
    )]
    pub pool_base_token_account: AccountInfo,

    /// SAFETY: same as `pool_base_token_account` above.
    #[account(
        mut,
        seeds = [pool.address().as_ref(), token_program.address().as_ref(), wsol_mint.address().as_ref()],
        seeds::program = ASSOCIATED_TOKEN_PROGRAM_ID,
        bump = args.pool_quote_token_account_bump,
    )]
    pub pool_quote_token_account: AccountInfo,
}

#[instruction]
pub fn migrate(ctx: Context<Migrate>, args: MigrateArgs) -> Result {
    let is_complete: bool = ctx.accounts.bonding_curve.complete.into();
    require!(is_complete, PumpError::NotComplete);

    // Real, confirmed via `reference/fee-tier-probe/src/bin/probe56.rs`
    // through `probe63.rs`: real `migrate` treats a curve whose four
    // reserve fields are all already zero as already migrated, and returns
    // successfully without running any of the CPI sequence below — proven
    // by isolating each field individually and in pairs (all disproven)
    // before confirming all four together is the real, exact condition,
    // independent of whether `pool`/`lp_mint` already exist.
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
    require!(
        ctx.accounts.bonding_curve.quote_mint == Address::default(),
        PumpError::UnsupportedQuoteMintForMigrate
    );

    let pool_migration_fee = ctx.accounts.global.pool_migration_fee;
    let base_token_amount = ctx.accounts.associated_bonding_curve.amount();
    let real_quote_reserves = ctx.accounts.bonding_curve.real_quote_reserves;
    require!(pool_migration_fee < real_quote_reserves, PumpError::PoolMigrationFeeTooHigh);
    let pool_quote_amount = real_quote_reserves - pool_migration_fee;

    let mint_address = ctx.accounts.mint.address();
    let bonding_curve_signer_seeds: &[&[u8]] =
        &[BONDING_CURVE_SEED, mint_address.as_ref(), &[args.bonding_curve_bump]];
    let bonding_curve_signer: &[&[&[u8]]] = &[bonding_curve_signer_seeds];

    let pool_authority_signer_seeds: &[&[u8]] =
        &[POOL_AUTHORITY_SEED, mint_address.as_ref(), &[args.pool_authority_bump]];
    let pool_authority_signer: &[&[&[u8]]] = &[pool_authority_signer_seeds];

    // `pool_migration_fee` funds `pool_authority`'s rent for every account it
    // pays to create below; the remainder is wrapped to WSOL further down.
    // Both raw credits here (and the raw debits paying for them) must be
    // reconciled by whatever CPI comes next, since Solana's runtime only
    // folds a raw (`sub_lamports`/`add_lamports`) lamport write into its own
    // tracked bookkeeping for accounts present in the very next CPI's own
    // account list — an unpaired raw credit is rejected as "unbalanced" the
    // moment any subsequent CPI's reconciliation sees it alone. Confirmed
    // against the real, deployed `pump.so` itself
    // (`reference/fee-tier-probe/src/bin/probe31.rs`'s decoded inner
    // instructions): every ATA-creation call and `SyncNative` there list
    // `bonding_curve` as an extra account they never actually read, purely
    // so its debit gets reconciled alongside whichever credit it funds.
    ctx.accounts.bonding_curve.sub_lamports(pool_migration_fee)?;
    ctx.accounts.pool_authority.add_lamports(pool_migration_fee)?;

    let bonding_curve_extra: [CpiHandle<'_>; 1] = [ctx.accounts.bonding_curve.info.to_cpi_handle()];
    let pool_authority_copy = ctx.accounts.pool_authority;

    // Order matches `probe31`'s decoded trace exactly: `pool`'s own ATAs
    // first, then `pool_authority`'s.
    naclac_lang::associated_token::create_idempotent_with_extra_accounts_signed(
        ctx.accounts.associated_token_program.to_cpi_handle(),
        naclac_lang::associated_token::AtaCpiAccounts {
            payer: ctx.accounts.pool_authority.to_cpi_handle_mut(),
            associated_token: ctx.accounts.pool_base_token_account.to_cpi_handle_mut(),
            authority: ctx.accounts.pool.to_cpi_handle(),
            mint: ctx.accounts.mint.to_cpi_handle(),
            system_program: ctx.accounts.system_program.to_cpi_handle(),
            token_program: ctx.accounts.token_program.to_cpi_handle(),
        },
        &bonding_curve_extra,
        pool_authority_signer,
    )?;
    naclac_lang::associated_token::create_idempotent_with_extra_accounts_signed(
        ctx.accounts.associated_token_program.to_cpi_handle(),
        naclac_lang::associated_token::AtaCpiAccounts {
            payer: ctx.accounts.pool_authority.to_cpi_handle_mut(),
            associated_token: ctx.accounts.pool_quote_token_account.to_cpi_handle_mut(),
            authority: ctx.accounts.pool.to_cpi_handle(),
            mint: ctx.accounts.wsol_mint.to_cpi_handle(),
            system_program: ctx.accounts.system_program.to_cpi_handle(),
            token_program: ctx.accounts.token_program.to_cpi_handle(),
        },
        &bonding_curve_extra,
        pool_authority_signer,
    )?;
    // `payer` and `authority` are both `pool_authority` here — `pool_authority_copy`
    // (a plain `Copy` of the `AccountInfo`) supplies the immutable `authority`
    // role so it doesn't alias the mutable `payer` borrow of the same field.
    naclac_lang::associated_token::create_idempotent_with_extra_accounts_signed(
        ctx.accounts.associated_token_program.to_cpi_handle(),
        naclac_lang::associated_token::AtaCpiAccounts {
            payer: ctx.accounts.pool_authority.to_cpi_handle_mut(),
            associated_token: ctx.accounts.pool_authority_mint_account.to_cpi_handle_mut(),
            authority: pool_authority_copy.to_cpi_handle(),
            mint: ctx.accounts.mint.to_cpi_handle(),
            system_program: ctx.accounts.system_program.to_cpi_handle(),
            token_program: ctx.accounts.token_program.to_cpi_handle(),
        },
        &bonding_curve_extra,
        pool_authority_signer,
    )?;
    naclac_lang::associated_token::create_idempotent_with_extra_accounts_signed(
        ctx.accounts.associated_token_program.to_cpi_handle(),
        naclac_lang::associated_token::AtaCpiAccounts {
            payer: ctx.accounts.pool_authority.to_cpi_handle_mut(),
            associated_token: ctx.accounts.pool_authority_wsol_account.to_cpi_handle_mut(),
            authority: pool_authority_copy.to_cpi_handle(),
            mint: ctx.accounts.wsol_mint.to_cpi_handle(),
            system_program: ctx.accounts.system_program.to_cpi_handle(),
            token_program: ctx.accounts.token_program.to_cpi_handle(),
        },
        &bonding_curve_extra,
        pool_authority_signer,
    )?;

    // `associated_bonding_curve`'s full balance moves untouched — confirmed
    // via probe, no fee taken on the base side during migration.
    let mint_decimals = ctx.accounts.mint.decimals();
    ctx.accounts.token_program.transfer_checked_signed(
        TransferCheckedAccounts {
            from: &mut ctx.accounts.associated_bonding_curve,
            mint: &ctx.accounts.mint,
            to: &mut ctx.accounts.pool_authority_mint_account,
            authority: &ctx.accounts.bonding_curve,
        },
        base_token_amount,
        mint_decimals,
        bonding_curve_signer,
    )?;

    ctx.accounts.bonding_curve.sub_lamports(pool_quote_amount)?;
    ctx.accounts.pool_authority_wsol_account.add_lamports(pool_quote_amount)?;

    naclac_lang::token::sync_native_with_extra_accounts(
        ctx.accounts.token_program.to_cpi_handle(),
        ctx.accounts.pool_authority_wsol_account.to_cpi_handle_mut(),
        ctx.accounts.rent.to_cpi_handle(),
        &bonding_curve_extra,
    )?;

    ctx.accounts.pump_amm.create_pool_signed(
        CreatePoolCpiAccounts {
            creator: ctx.accounts.pool_authority.to_cpi_handle_mut(),
            base_mint: ctx.accounts.mint.to_cpi_handle(),
            quote_mint: ctx.accounts.wsol_mint.to_cpi_handle(),
            global_config: ctx.accounts.amm_global_config.to_cpi_handle(),
            pool: ctx.accounts.pool.to_cpi_handle_mut(),
            lp_mint: ctx.accounts.lp_mint.to_cpi_handle_mut(),
            user_base_token_account: ctx.accounts.pool_authority_mint_account.to_cpi_handle_mut(),
            user_quote_token_account: ctx.accounts.pool_authority_wsol_account.to_cpi_handle_mut(),
            user_pool_token_account: ctx.accounts.user_pool_token_account.to_cpi_handle_mut(),
            pool_base_token_account: ctx.accounts.pool_base_token_account.to_cpi_handle_mut(),
            pool_quote_token_account: ctx.accounts.pool_quote_token_account.to_cpi_handle_mut(),
            system_program: ctx.accounts.system_program.to_cpi_handle(),
            token_2022_program: ctx.accounts.token_2022_program.to_cpi_handle(),
            base_token_program: ctx.accounts.token_program.to_cpi_handle(),
            quote_token_program: ctx.accounts.token_program.to_cpi_handle(),
            associated_token_program: ctx.accounts.associated_token_program.to_cpi_handle(),
        },
        PumpAmmCreatePoolArgs {
            index: 0,
            base_amount_in: base_token_amount,
            quote_amount_in: pool_quote_amount,
            coin_creator: ctx.accounts.bonding_curve.creator,
            is_mayhem_mode: ctx.accounts.bonding_curve.is_mayhem_mode,
            is_cashback_coin: ctx.accounts.bonding_curve.is_cashback_coin,
            pool_bump: args.pool_bump,
            lp_mint_bump: args.lp_mint_bump,
            user_base_token_account_bump: args.pool_authority_mint_account_bump,
            user_quote_token_account_bump: args.pool_authority_wsol_account_bump,
            user_pool_token_account_bump: args.user_pool_token_account_bump,
            pool_base_token_account_bump: args.pool_base_token_account_bump,
            pool_quote_token_account_bump: args.pool_quote_token_account_bump,
        },
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
    // `destination` and `authority` are the same account (`pool_authority`
    // reclaims its own ATA's rent) — `AccountInfo` is `Copy`, so a plain
    // copy (not `.clone()`) is enough to keep the mutable `destination`
    // borrow and the immutable `authority` borrow from aliasing.
    let pool_authority_for_close = ctx.accounts.pool_authority;
    ctx.accounts.token_2022_program.close_account_signed(
        CloseAccountAccounts {
            account: &mut ctx.accounts.user_pool_token_account,
            destination: &mut ctx.accounts.pool_authority,
            authority: &pool_authority_for_close,
        },
        pool_authority_signer,
    )?;

    // Any of `pool_migration_fee` left over after real rent costs sweeps to
    // `withdraw_authority` — confirmed via probe. `pool_authority` is a
    // plain, data-less System-owned PDA, so this leg is a real, tracked
    // `SystemProgram::transfer` CPI (not a raw lamport write), needing no
    // extra-accounts padding of its own.
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
        sol_amount: real_quote_reserves,
        pool_migration_fee,
        user: ctx.accounts.user.address(),
        mint: mint_address,
        bonding_curve: ctx.accounts.bonding_curve.address(),
        pool: ctx.accounts.pool.address(),
        quote_mint: WSOL_MINT,
    });

    msg!("Bonding curve successfully migrated");
    Ok(())
}
