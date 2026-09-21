use naclac_client::*;
use pump_amm_client::{
    fetch_global_config, fetch_pool, get_boost_vault_authority_pda, get_global_config_pda, get_lp_mint_pda,
    get_pool_pda,
    instructions::{
        build_boost_buy_and_burn, build_create_config, build_create_pool, build_init_boost,
        build_set_boost_authority, build_toggle_boost, build_transfer_creator_fees_to_pump,
        build_transfer_creator_fees_to_pump_v2, BoostBuyAndBurnAccounts, CreateConfigAccounts,
        CreatePoolAccounts, InitBoostAccounts, SetBoostAuthorityAccounts, ToggleBoostAccounts,
        TransferCreatorFeesToPumpAccounts, TransferCreatorFeesToPumpV2Accounts,
    },
    types::CreatePoolArgs,
    PROGRAM_ID,
};

fn setup() -> NaclacProvider {
    let payer = load_node_wallet().expect("Failed to load local Solana keypair");
    NaclacProvider::new("litesvm", payer).expect("Failed to construct NaclacProvider")
}

/// `create_config`'s golden path requires a signer matching `ADMIN_PUBKEY`
/// (a project-generated pubkey, not real `pump_amm`'s actual hardcoded
/// admin -- see that constant's own doc comment) -- reuses the exact same
/// keypair `pump_fees_test.rs`'s own `load_test_admin_keypair` uses, since
/// both `ADMIN_PUBKEY` constants are the same pubkey.
fn load_test_admin_keypair() -> Keypair {
    let mut path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    path.push("tests/wallets/test_admin.json");
    let file_content = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("Failed to read test admin keypair at {:?}: {}", path, e));
    let bytes: Vec<u8> = file_content
        .trim_matches(|c| c == '[' || c == ']' || c == ' ' || c == '\n' || c == '\r')
        .split(',')
        .map(|s| s.trim().parse::<u8>().unwrap())
        .collect();
    let secret_bytes: [u8; 32] = bytes[0..32].try_into().expect("keypair file too short");
    Keypair::new_from_array(secret_bytes)
}

/// `pump`'s real declared program ID â€” only used here as PDA seed-derivation
/// material for `pump_creator_vault`, never actually loaded (this test never
/// CPIs into `pump` itself, only `pump_amm` directly, to isolate whether
/// `transfer_creator_fees_to_pump(_v2)` works correctly on its own).
fn pump_program_id() -> Address {
    "FoN4cWC8wuVYK3Dd2ge1WVTLpPUvj4CcWXZsq4wmadwD"
        .parse()
        .expect("pump program id must parse")
}

fn wsol_mint_address() -> Address {
    "So11111111111111111111111111111111111111112"
        .parse()
        .expect("WSOL mint address must parse")
}

fn spl_mint_account_data(mint_authority: Option<&Address>, decimals: u8) -> Vec<u8> {
    let mut data = vec![0u8; 82];
    if let Some(auth) = mint_authority {
        data[0..4].copy_from_slice(&1u32.to_le_bytes());
        data[4..36].copy_from_slice(&auth.to_bytes());
    }
    data[36..44].copy_from_slice(&0u64.to_le_bytes());
    data[44] = decimals;
    data[45] = 1;
    data
}

fn spl_token_account_data(mint: &Address, owner: &Address, amount: u64, is_native_reserve: Option<u64>) -> Vec<u8> {
    let mut data = vec![0u8; 165];
    data[0..32].copy_from_slice(&mint.to_bytes());
    data[32..64].copy_from_slice(&owner.to_bytes());
    data[64..72].copy_from_slice(&amount.to_le_bytes());
    data[108] = 1;
    if let Some(reserve) = is_native_reserve {
        data[109..113].copy_from_slice(&1u32.to_le_bytes());
        data[113..121].copy_from_slice(&reserve.to_le_bytes());
    }
    data
}

const TOKEN_ACCOUNT_RENT_EXEMPT_RESERVE: u64 = 2_039_280;

fn read_token_account_amount(provider: &NaclacProvider, address: &Address) -> u64 {
    let data = provider.get_account_data(address).expect("token account should exist");
    u64::from_le_bytes(data[64..72].try_into().unwrap())
}

/// `GlobalConfig`'s real discriminator, per the generated IDL
/// (`target/idl/pump_amm.json`) â€” matches `reference/fee-tier-probe/src/bin/probe14.rs`'s
/// own `GLOBAL_CONFIG_DISCRIMINATOR` exactly (both are the same name-derived hash).
fn global_config_account_data(disable_flags: u8, boost_enabled: bool, admin: &Address, boost_authority: &Address) -> Vec<u8> {
    let mut data = vec![149, 8, 156, 202, 160, 252, 176, 217];
    data.push(0); // bump â€” unused by `create_pool`, value doesn't matter here
    data.push(disable_flags);
    data.push(if boost_enabled { 1 } else { 0 });
    data.extend_from_slice(&admin.to_bytes());
    data.extend_from_slice(&boost_authority.to_bytes());
    data
}

/// Isolates `transfer_creator_fees_to_pump_v2` from the rest of the CPI chain
/// `pump_fees` drives it through: calls it directly, with `coin_creator` an
/// arbitrary keypair (only ever used as PDA seed material, never
/// deserialized), and neither `coin_creator_vault_authority` nor
/// `pump_creator_vault` pre-created â€” mirrors exactly how `pump_fees` (and
/// `reference/fee-tier-probe/src/bin/probe11.rs`) leave them. Confirms
/// whether the real bytecode's close-recreate-forward sequence â€” including
/// the final direct `coin_creator_vault_authority.sub_lamports(..)` /
/// `pump_creator_vault.add_lamports(..)` step â€” works when
/// `coin_creator_vault_authority` was never explicitly created (still
/// System-owned going in).
#[test]
fn transfer_creator_fees_to_pump_v2_sweeps_wsol_balance() {
    let provider = setup();
    let payer = Keypair::new();
    provider.airdrop(&payer.address(), 10_000_000_000).unwrap();

    let coin_creator = Keypair::new().address();
    let wsol_mint = wsol_mint_address();
    provider
        .set_account(&wsol_mint, spl_mint_account_data(None, 9), &TOKEN_PROGRAM_ID, 10_000_000)
        .expect("inject WSOL mint fixture");

    let (coin_creator_vault_authority, coin_creator_vault_authority_bump) = Address::find_program_address(
        &[b"creator_vault", coin_creator.as_ref()],
        &PROGRAM_ID,
    );
    let (pump_creator_vault, pump_creator_vault_bump) = Address::find_program_address(
        &[b"creator-vault", coin_creator.as_ref()],
        &pump_program_id(),
    );

    let (coin_creator_vault_ata, _) = Address::find_program_address(
        &[coin_creator_vault_authority.as_ref(), TOKEN_PROGRAM_ID.as_ref(), wsol_mint.as_ref()],
        &ASSOCIATED_TOKEN_PROGRAM_ID,
    );
    const STARTING_WSOL_AMOUNT: u64 = 2_000_000_000;
    provider
        .set_account(
            &coin_creator_vault_ata,
            spl_token_account_data(&wsol_mint, &coin_creator_vault_authority, STARTING_WSOL_AMOUNT, Some(TOKEN_ACCOUNT_RENT_EXEMPT_RESERVE)),
            &TOKEN_PROGRAM_ID,
            TOKEN_ACCOUNT_RENT_EXEMPT_RESERVE + STARTING_WSOL_AMOUNT,
        )
        .expect("inject coin_creator_vault_ata fixture");

    let pump_creator_vault_ata = Keypair::new().address(); // dead account in this scoped WSOL pass

    let pump_creator_vault_balance_before = provider.get_balance(&pump_creator_vault).unwrap_or(0);

    build_transfer_creator_fees_to_pump_v2(
        &provider,
        PROGRAM_ID,
        coin_creator_vault_authority_bump,
        pump_creator_vault_bump,
        TransferCreatorFeesToPumpV2Accounts {
            payer: payer.address(),
            quote_mint: wsol_mint,
            token_program: TOKEN_PROGRAM_ID,
            system_program: SYSTEM_PROGRAM_ID,
            associated_token_program: ASSOCIATED_TOKEN_PROGRAM_ID,
            coin_creator,
            coin_creator_vault_authority,
            coin_creator_vault_ata,
            pump_creator_vault,
            pump_creator_vault_ata,
        },
    )
    .signer(&payer)
    .log()
    .send_and_confirm()
    .expect("transfer_creator_fees_to_pump_v2 should succeed");

    assert_eq!(
        read_token_account_amount(&provider, &coin_creator_vault_ata),
        0,
        "coin_creator_vault_ata should be fully swept"
    );
    let pump_creator_vault_balance_after = provider.get_balance(&pump_creator_vault).unwrap();
    assert!(
        pump_creator_vault_balance_after > pump_creator_vault_balance_before,
        "pump_creator_vault should have received the forwarded lamports"
    );
}

/// Same isolation, for the legacy WSOL-hardcoded v1 variant.
#[test]
fn transfer_creator_fees_to_pump_sweeps_wsol_balance() {
    let provider = setup();

    let coin_creator = Keypair::new().address();
    let wsol_mint = wsol_mint_address();
    provider
        .set_account(&wsol_mint, spl_mint_account_data(None, 9), &TOKEN_PROGRAM_ID, 10_000_000)
        .expect("inject WSOL mint fixture");

    let (coin_creator_vault_authority, coin_creator_vault_authority_bump) = Address::find_program_address(
        &[b"creator_vault", coin_creator.as_ref()],
        &PROGRAM_ID,
    );
    let (pump_creator_vault, pump_creator_vault_bump) = Address::find_program_address(
        &[b"creator-vault", coin_creator.as_ref()],
        &pump_program_id(),
    );

    let (coin_creator_vault_ata, _) = Address::find_program_address(
        &[coin_creator_vault_authority.as_ref(), TOKEN_PROGRAM_ID.as_ref(), wsol_mint.as_ref()],
        &ASSOCIATED_TOKEN_PROGRAM_ID,
    );
    const STARTING_WSOL_AMOUNT: u64 = 2_000_000_000;
    provider
        .set_account(
            &coin_creator_vault_ata,
            spl_token_account_data(&wsol_mint, &coin_creator_vault_authority, STARTING_WSOL_AMOUNT, Some(TOKEN_ACCOUNT_RENT_EXEMPT_RESERVE)),
            &TOKEN_PROGRAM_ID,
            TOKEN_ACCOUNT_RENT_EXEMPT_RESERVE + STARTING_WSOL_AMOUNT,
        )
        .expect("inject coin_creator_vault_ata fixture");

    let pump_creator_vault_balance_before = provider.get_balance(&pump_creator_vault).unwrap_or(0);

    build_transfer_creator_fees_to_pump(
        &provider,
        PROGRAM_ID,
        coin_creator_vault_authority_bump,
        pump_creator_vault_bump,
        TransferCreatorFeesToPumpAccounts {
            wsol_mint,
            token_program: TOKEN_PROGRAM_ID,
            system_program: SYSTEM_PROGRAM_ID,
            associated_token_program: ASSOCIATED_TOKEN_PROGRAM_ID,
            coin_creator,
            coin_creator_vault_authority,
            coin_creator_vault_ata,
            pump_creator_vault,
        },
    )
    .log()
    .send_and_confirm()
    .expect("transfer_creator_fees_to_pump should succeed");

    assert_eq!(read_token_account_amount(&provider, &coin_creator_vault_ata), 0);
    let pump_creator_vault_balance_after = provider.get_balance(&pump_creator_vault).unwrap();
    assert!(pump_creator_vault_balance_after > pump_creator_vault_balance_before);
}

/// End-to-end `create_pool`: real base mint (classic Token) + real WSOL quote
/// (classic Token), `base_token_program`/`quote_token_program` both classic
/// Token, `token_2022_program` used only for the LP mint/`user_pool_token_account`
/// (the one thing that's hardcoded, matching real `pump_amm.so`). Confirms the
/// LP bootstrap formula (`floor(sqrt(base*quote)) - 100`, `probe14`-confirmed),
/// that `pool_base_token_account`/`pool_quote_token_account` receive the full
/// deposit untouched (no fee on initial liquidity), and that `lp_mint` is
/// genuinely created under Token-2022 with the real 9-decimals value
/// (`probe14`-confirmed) despite `base_token_program`/`quote_token_program`
/// both being classic Token in the same instruction â€” the `token::program = X`
/// disambiguation fix this session added, exercised for real here.
#[test]
fn create_pool_bootstraps_liquidity_and_mints_lp_tokens() {
    let provider = setup();
    let creator = Keypair::new();
    provider.airdrop(&creator.address(), 10_000_000_000).unwrap();

    let base_mint = Keypair::new().address();
    provider
        .set_account(&base_mint, spl_mint_account_data(Some(&creator.address()), 6), &TOKEN_PROGRAM_ID, 10_000_000)
        .expect("inject base_mint fixture");

    let quote_mint = wsol_mint_address();
    provider
        .set_account(&quote_mint, spl_mint_account_data(None, 9), &TOKEN_PROGRAM_ID, 10_000_000)
        .expect("inject WSOL mint fixture");

    let (global_config, _global_config_bump) = get_global_config_pda(&PROGRAM_ID);
    provider
        .set_account(
            &global_config,
            global_config_account_data(0, false, &Address::default(), &Address::default()),
            &PROGRAM_ID,
            10_000_000,
        )
        .expect("inject global_config fixture (create_pool enabled)");

    const BASE_AMOUNT_IN: u64 = 1_000_000_000;
    const QUOTE_AMOUNT_IN: u64 = 2_000_000_000;

    let (user_base_token_account, user_base_token_account_bump) = Address::find_program_address(
        &[creator.address().as_ref(), TOKEN_PROGRAM_ID.as_ref(), base_mint.as_ref()],
        &ASSOCIATED_TOKEN_PROGRAM_ID,
    );
    provider
        .set_account(
            &user_base_token_account,
            spl_token_account_data(&base_mint, &creator.address(), BASE_AMOUNT_IN, None),
            &TOKEN_PROGRAM_ID,
            TOKEN_ACCOUNT_RENT_EXEMPT_RESERVE,
        )
        .expect("inject user_base_token_account fixture");

    let (user_quote_token_account, user_quote_token_account_bump) = Address::find_program_address(
        &[creator.address().as_ref(), TOKEN_PROGRAM_ID.as_ref(), quote_mint.as_ref()],
        &ASSOCIATED_TOKEN_PROGRAM_ID,
    );
    provider
        .set_account(
            &user_quote_token_account,
            spl_token_account_data(&quote_mint, &creator.address(), QUOTE_AMOUNT_IN, Some(TOKEN_ACCOUNT_RENT_EXEMPT_RESERVE)),
            &TOKEN_PROGRAM_ID,
            TOKEN_ACCOUNT_RENT_EXEMPT_RESERVE + QUOTE_AMOUNT_IN,
        )
        .expect("inject user_quote_token_account fixture");

    let index: u16 = 0;
    let (pool, pool_bump) = get_pool_pda(&PROGRAM_ID, index, &creator.address(), &base_mint, &quote_mint);
    let (lp_mint, lp_mint_bump) = get_lp_mint_pda(&PROGRAM_ID, &pool);

    let (user_pool_token_account, _user_pool_token_account_bump) = Address::find_program_address(
        &[creator.address().as_ref(), TOKEN_2022_PROGRAM_ID.as_ref(), lp_mint.as_ref()],
        &ASSOCIATED_TOKEN_PROGRAM_ID,
    );
    let (pool_base_token_account, _pool_base_token_account_bump) = Address::find_program_address(
        &[pool.as_ref(), TOKEN_PROGRAM_ID.as_ref(), base_mint.as_ref()],
        &ASSOCIATED_TOKEN_PROGRAM_ID,
    );
    let (pool_quote_token_account, _pool_quote_token_account_bump) = Address::find_program_address(
        &[pool.as_ref(), TOKEN_PROGRAM_ID.as_ref(), quote_mint.as_ref()],
        &ASSOCIATED_TOKEN_PROGRAM_ID,
    );

    let coin_creator = Keypair::new().address();

    build_create_pool(
        &provider,
        PROGRAM_ID,
        CreatePoolArgs {
            index,
            base_amount_in: BASE_AMOUNT_IN,
            quote_amount_in: QUOTE_AMOUNT_IN,
            coin_creator,
            is_mayhem_mode: Bool::from(false),
            is_cashback_coin: Bool::from(false),
            pool_bump,
            lp_mint_bump,
            user_base_token_account_bump,
            user_quote_token_account_bump,
            ..Default::default()
        },
        CreatePoolAccounts {
            creator: creator.address(),
            base_mint,
            quote_mint,
            global_config,
            pool,
            lp_mint,
            user_base_token_account,
            user_quote_token_account,
            user_pool_token_account,
            pool_base_token_account,
            pool_quote_token_account,
            system_program: SYSTEM_PROGRAM_ID,
            token_2022_program: TOKEN_2022_PROGRAM_ID,
            base_token_program: TOKEN_PROGRAM_ID,
            quote_token_program: TOKEN_PROGRAM_ID,
            associated_token_program: ASSOCIATED_TOKEN_PROGRAM_ID,
        },
    )
    .signer(&creator)
    .log()
    .send_and_confirm()
    .expect("create_pool should succeed");

    assert_eq!(
        read_token_account_amount(&provider, &pool_base_token_account),
        BASE_AMOUNT_IN,
        "pool_base_token_account should hold the full deposit, no fee on initial liquidity"
    );
    assert_eq!(
        read_token_account_amount(&provider, &pool_quote_token_account),
        QUOTE_AMOUNT_IN,
        "pool_quote_token_account should hold the full deposit, no fee on initial liquidity"
    );

    // floor(sqrt(1e9 * 2e9)) - 100 = floor(sqrt(2e18)) - 100 = 1_414_213_562 - 100
    // = 1_414_213_462 â€” same formula/inputs `probe14` confirmed against real bytecode.
    const EXPECTED_LP_MINTED: u64 = 1_414_213_462;
    assert_eq!(
        read_token_account_amount(&provider, &user_pool_token_account),
        EXPECTED_LP_MINTED,
        "user_pool_token_account should receive the full LP bootstrap amount"
    );

    let lp_mint_account = provider.get_account(&lp_mint).expect("lp_mint should exist");
    assert_eq!(
        lp_mint_account.owner, TOKEN_2022_PROGRAM_ID,
        "lp_mint must be owned by Token-2022 even though base/quote_token_program are classic Token \
         in this same instruction â€” the token::program disambiguation fix, exercised for real"
    );
    assert_eq!(lp_mint_account.data[44], 9, "lp_mint decimals must be 9 (probe14-confirmed real value)");

    let pool_account = fetch_pool(&provider, &pool).expect("pool should be readable");
    assert_eq!(pool_account.pool_bump, pool_bump);
    assert_eq!(pool_account.index, index);
    assert_eq!(pool_account.creator, creator.address());
    assert_eq!(pool_account.base_mint, base_mint);
    assert_eq!(pool_account.quote_mint, quote_mint);
    assert_eq!(pool_account.lp_mint, lp_mint);
    assert_eq!(pool_account.coin_creator, coin_creator);
    assert_eq!(pool_account.lp_supply, EXPECTED_LP_MINTED);

    let global_config_account = fetch_global_config(&provider, &global_config).expect("global_config should be readable");
    assert_eq!(global_config_account.disable_flags, 0);
}

/// End-to-end `init_boost` -> `boost_buy_and_burn` chained onto a real
/// `create_pool`. `EXPECTED_VIRTUAL_QUOTE_RESERVES` uses the real formula
/// confirmed against 5 independent real mainnet transactions plus a direct
/// blind litesvm replication (`docs/plan/amm-03-boost-mechanism.md`):
/// `virtual_quote_reserves = floor(quote_balance * base_balance / 1e15)`.
#[test]
fn init_boost_and_boost_buy_and_burn_end_to_end() {
    let provider = setup();
    let creator = Keypair::new();
    provider.airdrop(&creator.address(), 10_000_000_000).unwrap();

    let base_mint = Keypair::new().address();
    provider
        .set_account(&base_mint, spl_mint_account_data(Some(&creator.address()), 6), &TOKEN_PROGRAM_ID, 10_000_000)
        .expect("inject base_mint fixture");

    let quote_mint = wsol_mint_address();
    provider
        .set_account(&quote_mint, spl_mint_account_data(None, 9), &TOKEN_PROGRAM_ID, 10_000_000)
        .expect("inject WSOL mint fixture");

    let admin = Keypair::new();
    let boost_authority = Keypair::new();
    let (global_config, _global_config_bump) = get_global_config_pda(&PROGRAM_ID);
    provider
        .set_account(
            &global_config,
            global_config_account_data(0, true, &admin.address(), &boost_authority.address()),
            &PROGRAM_ID,
            10_000_000,
        )
        .expect("inject global_config fixture (boost enabled)");

    const BASE_AMOUNT_IN: u64 = 200_000_000_000_000;
    const QUOTE_AMOUNT_IN: u64 = 17_300_000_000;

    let (user_base_token_account, user_base_token_account_bump) = Address::find_program_address(
        &[creator.address().as_ref(), TOKEN_PROGRAM_ID.as_ref(), base_mint.as_ref()],
        &ASSOCIATED_TOKEN_PROGRAM_ID,
    );
    provider
        .set_account(
            &user_base_token_account,
            spl_token_account_data(&base_mint, &creator.address(), BASE_AMOUNT_IN, None),
            &TOKEN_PROGRAM_ID,
            TOKEN_ACCOUNT_RENT_EXEMPT_RESERVE,
        )
        .expect("inject user_base_token_account fixture");

    let (user_quote_token_account, user_quote_token_account_bump) = Address::find_program_address(
        &[creator.address().as_ref(), TOKEN_PROGRAM_ID.as_ref(), quote_mint.as_ref()],
        &ASSOCIATED_TOKEN_PROGRAM_ID,
    );
    provider
        .set_account(
            &user_quote_token_account,
            spl_token_account_data(&quote_mint, &creator.address(), QUOTE_AMOUNT_IN, Some(TOKEN_ACCOUNT_RENT_EXEMPT_RESERVE)),
            &TOKEN_PROGRAM_ID,
            TOKEN_ACCOUNT_RENT_EXEMPT_RESERVE + QUOTE_AMOUNT_IN,
        )
        .expect("inject user_quote_token_account fixture");

    let index: u16 = 0;
    let (pool, pool_bump) = get_pool_pda(&PROGRAM_ID, index, &creator.address(), &base_mint, &quote_mint);
    let (lp_mint, lp_mint_bump) = get_lp_mint_pda(&PROGRAM_ID, &pool);

    let (user_pool_token_account, _user_pool_token_account_bump) = Address::find_program_address(
        &[creator.address().as_ref(), TOKEN_2022_PROGRAM_ID.as_ref(), lp_mint.as_ref()],
        &ASSOCIATED_TOKEN_PROGRAM_ID,
    );
    let (pool_base_token_account, _pool_base_token_account_bump) = Address::find_program_address(
        &[pool.as_ref(), TOKEN_PROGRAM_ID.as_ref(), base_mint.as_ref()],
        &ASSOCIATED_TOKEN_PROGRAM_ID,
    );
    let (pool_quote_token_account, _pool_quote_token_account_bump) = Address::find_program_address(
        &[pool.as_ref(), TOKEN_PROGRAM_ID.as_ref(), quote_mint.as_ref()],
        &ASSOCIATED_TOKEN_PROGRAM_ID,
    );

    let coin_creator = Keypair::new().address();

    build_create_pool(
        &provider,
        PROGRAM_ID,
        CreatePoolArgs {
            index,
            base_amount_in: BASE_AMOUNT_IN,
            quote_amount_in: QUOTE_AMOUNT_IN,
            coin_creator,
            is_mayhem_mode: Bool::from(false),
            is_cashback_coin: Bool::from(false),
            pool_bump,
            lp_mint_bump,
            user_base_token_account_bump,
            user_quote_token_account_bump,
            ..Default::default()
        },
        CreatePoolAccounts {
            creator: creator.address(),
            base_mint,
            quote_mint,
            global_config,
            pool,
            lp_mint,
            user_base_token_account,
            user_quote_token_account,
            user_pool_token_account,
            pool_base_token_account,
            pool_quote_token_account,
            system_program: SYSTEM_PROGRAM_ID,
            token_2022_program: TOKEN_2022_PROGRAM_ID,
            base_token_program: TOKEN_PROGRAM_ID,
            quote_token_program: TOKEN_PROGRAM_ID,
            associated_token_program: ASSOCIATED_TOKEN_PROGRAM_ID,
        },
    )
    .signer(&creator)
    .log()
    .send_and_confirm()
    .expect("create_pool should succeed");

    let bonding_curve = Keypair::new().address(); // never dereferenced, only logged

    let (boost_vault_authority, boost_vault_authority_bump) = get_boost_vault_authority_pda(&PROGRAM_ID, &pool);
    let (boost_vault, _boost_vault_bump) = Address::find_program_address(
        &[boost_vault_authority.as_ref(), TOKEN_PROGRAM_ID.as_ref(), quote_mint.as_ref()],
        &ASSOCIATED_TOKEN_PROGRAM_ID,
    );

    build_init_boost(
        &provider,
        PROGRAM_ID,
        boost_vault_authority_bump,
        InitBoostAccounts {
            bonding_curve,
            pool,
            global_config,
            creator: creator.address(),
            base_mint,
            quote_mint,
            pool_base_token_account,
            pool_quote_token_account,
            boost_vault_authority,
            boost_vault,
            quote_token_program: TOKEN_PROGRAM_ID,
            system_program: SYSTEM_PROGRAM_ID,
            associated_token_program: ASSOCIATED_TOKEN_PROGRAM_ID,
        },
    )
    .signer(&creator)
    .log()
    .send_and_confirm()
    .expect("init_boost should succeed");

    const EXPECTED_VIRTUAL_QUOTE_RESERVES: u64 = 3_460_000_000; // 17.3e9 * 200e12 / 1e15
    assert_eq!(
        read_token_account_amount(&provider, &boost_vault),
        EXPECTED_VIRTUAL_QUOTE_RESERVES,
        "boost_vault should hold exactly the computed virtual_quote_reserves"
    );
    assert_eq!(
        read_token_account_amount(&provider, &pool_quote_token_account),
        QUOTE_AMOUNT_IN - EXPECTED_VIRTUAL_QUOTE_RESERVES,
        "pool_quote_token_account should be debited by exactly the transferred amount"
    );
    let pool_account = fetch_pool(&provider, &pool).expect("pool should be readable");
    assert_eq!(
        i128::from_le_bytes(pool_account.virtual_quote_reserves),
        EXPECTED_VIRTUAL_QUOTE_RESERVES as i128
    );

    let base_before = read_token_account_amount(&provider, &pool_base_token_account);
    let quote_before = read_token_account_amount(&provider, &pool_quote_token_account);
    let boost_vault_before = read_token_account_amount(&provider, &boost_vault);

    provider.airdrop(&boost_authority.address(), 10_000_000_000).unwrap();
    const QUOTE_AMOUNT_IN_FOR_BOOST: u64 = 500_000_000;

    build_boost_buy_and_burn(
        &provider,
        PROGRAM_ID,
        QUOTE_AMOUNT_IN_FOR_BOOST,
        0,
        boost_vault_authority_bump,
        BoostBuyAndBurnAccounts {
            bonding_curve,
            pool,
            authority: boost_authority.address(),
            global_config,
            base_mint,
            quote_mint,
            pool_base_token_account,
            pool_quote_token_account,
            boost_vault_authority,
            boost_vault,
            base_token_program: TOKEN_PROGRAM_ID,
            quote_token_program: TOKEN_PROGRAM_ID,
        },
    )
    .signer(&boost_authority)
    .log()
    .send_and_confirm()
    .expect("boost_buy_and_burn should succeed");

    let base_after = read_token_account_amount(&provider, &pool_base_token_account);
    let quote_after = read_token_account_amount(&provider, &pool_quote_token_account);
    let boost_vault_after = read_token_account_amount(&provider, &boost_vault);

    assert_eq!(
        quote_after,
        quote_before + QUOTE_AMOUNT_IN_FOR_BOOST,
        "pool_quote_token_account should receive the full spent amount"
    );
    assert_eq!(
        boost_vault_after,
        boost_vault_before - QUOTE_AMOUNT_IN_FOR_BOOST,
        "boost_vault should be debited by exactly the spent amount"
    );
    assert!(base_after < base_before, "some base tokens should have been burned from pool_base_token_account");

    let pool_account_after = fetch_pool(&provider, &pool).expect("pool should be readable");
    assert_eq!(
        i128::from_le_bytes(pool_account_after.virtual_quote_reserves),
        EXPECTED_VIRTUAL_QUOTE_RESERVES as i128,
        "virtual_quote_reserves must stay unchanged by boost_buy_and_burn"
    );
}

#[test]
fn toggle_boost_flips_global_config_flag() {
    let provider = setup();
    let admin = Keypair::new();
    provider.airdrop(&admin.address(), 10_000_000_000).unwrap();
    let boost_authority = Keypair::new().address();

    let (global_config, _bump) = get_global_config_pda(&PROGRAM_ID);
    provider
        .set_account(
            &global_config,
            global_config_account_data(0, false, &admin.address(), &boost_authority),
            &PROGRAM_ID,
            10_000_000,
        )
        .expect("inject global_config fixture");

    build_toggle_boost(
        &provider,
        PROGRAM_ID,
        Bool::from(true),
        ToggleBoostAccounts { admin: admin.address(), global_config },
    )
    .signer(&admin)
    .log()
    .send_and_confirm()
    .expect("toggle_boost should succeed");

    let global_config_account = fetch_global_config(&provider, &global_config).expect("global_config should be readable");
    assert!(bool::from(global_config_account.boost_enabled));
}

#[test]
fn set_boost_authority_updates_global_config() {
    let provider = setup();
    let admin = Keypair::new();
    provider.airdrop(&admin.address(), 10_000_000_000).unwrap();
    let old_boost_authority = Keypair::new().address();
    let new_boost_authority = Keypair::new().address();

    let (global_config, _bump) = get_global_config_pda(&PROGRAM_ID);
    provider
        .set_account(
            &global_config,
            global_config_account_data(0, false, &admin.address(), &old_boost_authority),
            &PROGRAM_ID,
            10_000_000,
        )
        .expect("inject global_config fixture");

    build_set_boost_authority(
        &provider,
        PROGRAM_ID,
        SetBoostAuthorityAccounts {
            admin: admin.address(),
            global_config,
            boost_authority: new_boost_authority,
            system_program: SYSTEM_PROGRAM_ID,
        },
    )
    .signer(&admin)
    .log()
    .send_and_confirm()
    .expect("set_boost_authority should succeed");

    let global_config_account = fetch_global_config(&provider, &global_config).expect("global_config should be readable");
    assert_eq!(global_config_account.boost_authority, new_boost_authority);
}

/// `create_config` was added specifically because real `pump_amm` has no
/// other way to bring `GlobalConfig` into existence on a real cluster --
/// every other test in this file injects it via raw `set_account` bytes,
/// which only works on litesvm. Proves the real instruction chain a devnet
/// deployment will actually need: `create_config` (creates it, admin-gated,
/// boost left disabled) -> `toggle_boost` (enables it) -> `set_boost_authority`
/// (sets a real boost authority) -- all real instructions, no fixture
/// injection at all.
#[test]
fn create_config_then_toggle_boost_then_set_boost_authority_via_real_instructions() {
    let provider = setup();
    let admin = load_test_admin_keypair();
    provider.airdrop(&admin.address(), 10_000_000_000).unwrap();

    let (global_config, _bump) = get_global_config_pda(&PROGRAM_ID);

    build_create_config(
        &provider,
        PROGRAM_ID,
        CreateConfigAccounts { admin: admin.address(), global_config, system_program: SYSTEM_PROGRAM_ID },
    )
    .signer(&admin)
    .log()
    .send_and_confirm()
    .expect("create_config should succeed for the real ADMIN_PUBKEY signer");

    let created = fetch_global_config(&provider, &global_config).expect("global_config should be readable");
    assert_eq!(created.admin, admin.address());
    assert_eq!(created.disable_flags, 0);
    assert!(!bool::from(created.boost_enabled), "boost should start disabled -- create_config's real args don't include it");
    assert_eq!(created.boost_authority, Address::default());

    build_toggle_boost(
        &provider,
        PROGRAM_ID,
        Bool::from(true),
        ToggleBoostAccounts { admin: admin.address(), global_config },
    )
    .signer(&admin)
    .log()
    .send_and_confirm()
    .expect("toggle_boost should succeed");

    let boost_authority = Keypair::new().address();
    build_set_boost_authority(
        &provider,
        PROGRAM_ID,
        SetBoostAuthorityAccounts {
            admin: admin.address(),
            global_config,
            boost_authority,
            system_program: SYSTEM_PROGRAM_ID,
        },
    )
    .signer(&admin)
    .log()
    .send_and_confirm()
    .expect("set_boost_authority should succeed");

    let final_config = fetch_global_config(&provider, &global_config).expect("global_config should be readable");
    assert!(bool::from(final_config.boost_enabled), "boost should be enabled after toggle_boost");
    assert_eq!(final_config.boost_authority, boost_authority);
}
