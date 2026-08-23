use naclac_client::*;
use pump_client::{
    fetch_bonding_curve, fetch_global, get_bonding_curve_pda, get_global_pda,
    get_metadata_pda, get_mint_authority_pda,
    instructions::{
        build_add_quote_mint, build_buy, build_buy_v2, build_create, build_create_v2,
        build_initialize, build_migrate, build_migrate_v2, build_remove_quote_mint, build_sell,
        build_sell_v2, build_set_creator, build_set_metaplex_creator, build_set_params,
        build_set_reserved_fee_recipients,
        build_set_virtual_quote_reserves,
        build_toggle_cashback_enabled, build_toggle_create_v2, build_update_buyback_config,
        AddQuoteMintAccounts, BuyAccounts, BuyV2Accounts,
        CreateAccounts, CreateV2Accounts, InitializeAccounts, MigrateAccounts, MigrateV2Accounts,
        RemoveQuoteMintAccounts, SellAccounts, SellV2Accounts, SetCreatorAccounts,
        SetMetaplexCreatorAccounts,
        SetParamsAccounts, SetReservedFeeRecipientsAccounts, SetVirtualQuoteReservesAccounts,
        ToggleCashbackEnabledAccounts,
        ToggleCreateV2Accounts, UpdateBuybackConfigAccounts,
    },
    types::{BuyV2Args, MigrateV2Args, SellV2Args},
    BOOST_VAULT_SEED, BONDING_CURVE_V2_SEED, BUYBACK_VAULT_SEED, CREATOR_VAULT_RENT_EXEMPT_MINIMUM,
    CREATOR_VAULT_SEED, FEE_CONFIG_SEED, GLOBAL_CONFIG_SEED,
    GLOBAL_VOLUME_ACCUMULATOR_SEED, MPL_TOKEN_METADATA_PROGRAM_ID, POOL_AUTHORITY_SEED, POOL_LP_MINT_SEED,
    POOL_SEED, PROGRAM_ID, PUMP_AMM_PROGRAM_ID, PUMP_AUTHORITY_SEED, PUMP_FEES_PROGRAM_ID,
    USER_VOLUME_ACCUMULATOR_SEED,
};

/// Mirrors `pump_fees_test.rs`'s helper of the same name and shape — asserts
/// the exact numeric `NaclacError` code, not just "any error".
fn assert_custom_code(result: Result<NaclacTransactionMetadata, NaclacClientError>, expected: u32) {
    match result {
        Err(NaclacClientError::TransactionFailed {
            instruction_err: InstructionError::Custom(code),
            ..
        }) => {
            assert_eq!(code, expected, "wrong custom error code");
        }
        other => panic!("expected a Custom({expected}) transaction failure, got {other:?}"),
    }
}

/// `setup_tradeable_bonding_curve`'s injected `FeeConfig`'s single zero-threshold
/// tier — every trade test in this file is priced against these exact bps.
const TEST_PROTOCOL_FEE_BPS: u64 = 100;
const TEST_CREATOR_FEE_BPS: u64 = 50;
/// `setup_fee_config`'s `stable_fee_tiers` entry -- deliberately different
/// from `TEST_PROTOCOL_FEE_BPS`/`TEST_CREATOR_FEE_BPS` above, not mirrored,
/// so a non-SOL-quote test asserting against these values actually proves
/// `get_fees` read `stable_fee_tiers` rather than `fee_tiers` (`is_new_quote_mint`
/// wired correctly), instead of passing either way because both tables agree.
const TEST_STABLE_PROTOCOL_FEE_BPS: u64 = 150;
const TEST_STABLE_CREATOR_FEE_BPS: u64 = 75;
/// `Global.buyback_basis_points` is left at its zero-initialized default by
/// `initialize` (never set by any test helper) — the buyback split is always
/// 0 in this file's fixtures, so `fee_recipient` always receives the full
/// `protocol_fee`.
const TEST_BUYBACK_BASIS_POINTS: u64 = 0;

/// Mirrors `systems/swap_math.rs`'s `gross_sol_for_tokens_buy` exactly
/// (`#[system(rounding = "up")]`: `ceil(a / b) = (a + b - 1) / b`), so tests
/// can independently compute the expected quote amount for a `buy`/`buy_v2`
/// trade and assert real on-chain balances against it, not just direction.
fn expected_gross_buy(tokens: u128, virtual_quote_reserves: u128, virtual_token_reserves: u128) -> u64 {
    let numerator = tokens * virtual_quote_reserves;
    let denominator = virtual_token_reserves - tokens;
    ((numerator + denominator - 1) / denominator) as u64
}

/// Mirrors `systems/swap_math.rs`'s `gross_sol_for_tokens_sell` exactly
/// (`#[system(rounding = "down")]`: plain floor division).
fn expected_gross_sell(tokens: u128, virtual_quote_reserves: u128, virtual_token_reserves: u128) -> u64 {
    ((tokens * virtual_quote_reserves) / (virtual_token_reserves + tokens)) as u64
}

/// Mirrors `systems/swap_math.rs`'s `fee_amount_ceil` exactly.
fn expected_fee_ceil(amount: u128, basis_points: u64) -> u64 {
    ((amount * basis_points as u128 + 9_999) / 10_000) as u64
}

/// Mirrors `systems/swap_math.rs`'s `fee_amount_floor` exactly.
fn expected_fee_floor(amount: u128, basis_points: u64) -> u64 {
    ((amount * basis_points as u128) / 10_000) as u64
}

/// Real `pump.so` validates `bonding_curve_v2`'s address on every classic
/// `buy`/`sell` call even though the account itself never needs to exist —
/// confirmed via `reference/fee-tier-probe/src/bin/probe52.rs`/`probe53.rs`.
fn bonding_curve_v2_pda(mint: &Address) -> (Address, u8) {
    Address::find_program_address(&[BONDING_CURVE_V2_SEED, mint.as_ref()], &PROGRAM_ID)
}

fn token_balance(provider: &NaclacProvider, address: &Address) -> u64 {
    let data = provider.get_account_data(address).expect("token account should exist");
    u64::from_le_bytes(data[64..72].try_into().unwrap())
}

fn load_program(provider: &NaclacProvider) {
    let mut workspace_root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    workspace_root.pop(); // programs
    workspace_root.pop(); // pump-bonding-curve workspace root
    let so_path = resolve_cargo_target_dir(&workspace_root).join("deploy/pump.so");

    provider
        .add_program(&PROGRAM_ID, so_path.to_str().unwrap())
        .expect("Failed to load pump.so");
}

/// `create` genuinely CPIs into real Metaplex Token Metadata
/// (`create_metadata_via_cpi`), so every test exercising `create` needs the
/// real program loaded — there's no naclac-generated client for it, so
/// there's nothing to mock it out with.
fn load_mpl_token_metadata_program(provider: &NaclacProvider) {
    let mut so_path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    so_path.pop(); // programs
    so_path.pop(); // pump-bonding-curve workspace root
    so_path.pop(); // pump-fun-program
    so_path.push("reference/pump-rust-client/artifacts/mpl_token_metadata.so");

    provider
        .add_program(&MPL_TOKEN_METADATA_PROGRAM_ID, so_path.to_str().unwrap())
        .expect("Failed to load mpl_token_metadata.so");
}

fn setup() -> NaclacProvider {
    let payer = load_node_wallet().expect("Failed to load local Solana keypair");
    let provider = NaclacProvider::new("litesvm", payer);
    load_program(&provider);
    load_mpl_token_metadata_program(&provider);
    provider
}

fn init_global(provider: &NaclacProvider) -> (Address, Keypair) {
    let user = Keypair::new();
    provider.airdrop(&user.address(), 10_000_000_000).unwrap();
    let (global_pda, _bump) = get_global_pda(&PROGRAM_ID);

    build_initialize(
        provider,
        PROGRAM_ID,
        InitializeAccounts {
            user: user.address(),
            global: global_pda,
            system_program: SYSTEM_PROGRAM_ID,
        },
    )
    .signer(&user)
    .log()
    .send_and_confirm()
    .expect("initialize should succeed");

    (global_pda, user)
}

fn create_bonding_curve(
    provider: &NaclacProvider,
    global_pda: Address,
    creator: Address,
) -> (Address, Keypair, u8) {
    let user = Keypair::new();
    provider.airdrop(&user.address(), 10_000_000_000).unwrap();
    let mint = Keypair::new();
    let (bonding_curve_pda, bonding_curve_bump) = get_bonding_curve_pda(&PROGRAM_ID, &mint.address());
    let (mint_authority_pda, _) = get_mint_authority_pda(&PROGRAM_ID);
    let (associated_bonding_curve_pda, _) = Address::find_program_address(
        &[bonding_curve_pda.as_ref(), TOKEN_PROGRAM_ID.as_ref(), mint.address().as_ref()],
        &ASSOCIATED_TOKEN_PROGRAM_ID,
    );
    let (metadata_pda, metadata_bump) = get_metadata_pda(&mint.address());

    build_create(
        provider,
        PROGRAM_ID,
        "Test Coin".to_string(),
        "TEST".to_string(),
        "https://example.com/metadata.json".to_string(),
        creator,
        bonding_curve_bump,
        metadata_bump,
        CreateAccounts {
            user: user.address(),
            mint_authority: mint_authority_pda,
            mint: mint.address(),
            bonding_curve: bonding_curve_pda,
            associated_bonding_curve: associated_bonding_curve_pda,
            global: global_pda,
            mpl_token_metadata: MPL_TOKEN_METADATA_PROGRAM_ID,
            metadata: metadata_pda,
            system_program: SYSTEM_PROGRAM_ID,
            token_program: TOKEN_PROGRAM_ID,
            associated_token_program: ASSOCIATED_TOKEN_PROGRAM_ID,
        },
    )
    .signer(&user)
    .signer(&mint)
    .log()
    .send_and_confirm()
    .expect("create should succeed");

    (bonding_curve_pda, mint, bonding_curve_bump)
}

fn read_mint_decimals(provider: &NaclacProvider, address: &Address) -> u8 {
    let data = provider.get_account_data(address).expect("mint account should exist");
    data[44]
}

fn create_bonding_curve_v2(
    provider: &NaclacProvider,
    global_pda: Address,
    creator: Address,
    is_cashback_enabled: Bool,
) -> (Address, Keypair, u8) {
    let user = Keypair::new();
    provider.airdrop(&user.address(), 10_000_000_000).unwrap();
    let mint = Keypair::new();
    let (bonding_curve_pda, bonding_curve_bump) = get_bonding_curve_pda(&PROGRAM_ID, &mint.address());
    let (mint_authority_pda, _) = get_mint_authority_pda(&PROGRAM_ID);
    let (associated_bonding_curve_pda, _) = Address::find_program_address(
        &[bonding_curve_pda.as_ref(), TOKEN_2022_PROGRAM_ID.as_ref(), mint.address().as_ref()],
        &ASSOCIATED_TOKEN_PROGRAM_ID,
    );

    build_create_v2(
        provider,
        PROGRAM_ID,
        "Test Coin V2".to_string(),
        "TESTV2".to_string(),
        "https://example.com/metadata-v2.json".to_string(),
        creator,
        is_cashback_enabled,
        bonding_curve_bump,
        CreateV2Accounts {
            user: user.address(),
            mint: mint.address(),
            mint_authority: mint_authority_pda,
            bonding_curve: bonding_curve_pda,
            associated_bonding_curve: associated_bonding_curve_pda,
            global: global_pda,
            system_program: SYSTEM_PROGRAM_ID,
            token_program: TOKEN_2022_PROGRAM_ID,
            associated_token_program: ASSOCIATED_TOKEN_PROGRAM_ID,
            quote_mint: None,
            quote_token_program: None,
            associated_quote_bonding_curve: None,
        },
    )
    .signer(&user)
    .signer(&mint)
    .log()
    .send_and_confirm()
    .expect("create_v2 should succeed");

    (bonding_curve_pda, mint, bonding_curve_bump)
}

/// Same as `create_bonding_curve_v2`, but passes `create_v2`'s 3 optional
/// non-SOL-quote accounts (`quote_mint`/`quote_token_program`/
/// `associated_quote_bonding_curve` -- naclac `Option<T>` account fields,
/// not `remaining_accounts`; see `create_v2.rs`'s module comment) and
/// returns the raw `Result` instead of `.expect()`-ing it, so callers can
/// assert either a successful (whitelisted) or failing (unwhitelisted)
/// outcome. `quote_token_program`/`associated_quote_bonding_curve` are
/// derived directly from `quote_mint` being present -- the generated
/// `CreateV2Accounts.quote_mint`/etc. fields handle the `None` sentinel
/// automatically, so no manual `AccountMeta`/`remaining_accounts()` needed.
fn create_bonding_curve_v2_with_quote_mint(
    provider: &NaclacProvider,
    global_pda: Address,
    creator: Address,
    quote_mint: Option<Address>,
) -> (Result<NaclacTransactionMetadata, NaclacClientError>, Address, Keypair) {
    let user = Keypair::new();
    provider.airdrop(&user.address(), 10_000_000_000).unwrap();
    let mint = Keypair::new();
    let (bonding_curve_pda, bonding_curve_bump) = get_bonding_curve_pda(&PROGRAM_ID, &mint.address());
    let (mint_authority_pda, _) = get_mint_authority_pda(&PROGRAM_ID);
    let (associated_bonding_curve_pda, _) = Address::find_program_address(
        &[bonding_curve_pda.as_ref(), TOKEN_2022_PROGRAM_ID.as_ref(), mint.address().as_ref()],
        &ASSOCIATED_TOKEN_PROGRAM_ID,
    );
    let associated_quote_bonding_curve_pda = quote_mint.map(|qm| {
        Address::find_program_address(
            &[bonding_curve_pda.as_ref(), TOKEN_PROGRAM_ID.as_ref(), qm.as_ref()],
            &ASSOCIATED_TOKEN_PROGRAM_ID,
        )
        .0
    });

    let result = build_create_v2(
        provider,
        PROGRAM_ID,
        "Test Coin V2".to_string(),
        "TESTV2".to_string(),
        "https://example.com/metadata-v2.json".to_string(),
        creator,
        Bool::from(false),
        bonding_curve_bump,
        CreateV2Accounts {
            user: user.address(),
            mint: mint.address(),
            mint_authority: mint_authority_pda,
            bonding_curve: bonding_curve_pda,
            associated_bonding_curve: associated_bonding_curve_pda,
            global: global_pda,
            system_program: SYSTEM_PROGRAM_ID,
            token_program: TOKEN_2022_PROGRAM_ID,
            associated_token_program: ASSOCIATED_TOKEN_PROGRAM_ID,
            quote_mint,
            quote_token_program: quote_mint.map(|_| TOKEN_PROGRAM_ID),
            associated_quote_bonding_curve: associated_quote_bonding_curve_pda,
        },
    )
    .signer(&user)
    .signer(&mint)
    .log()
    .send_and_confirm();

    (result, bonding_curve_pda, mint)
}

/// Hand-parses the real Token-2022 `TokenMetadata` TLV entry directly out of
/// a mint's raw account bytes (TYPE=19, confirmed against
/// `spl-token-2022-interface-3.1.1`'s real `ExtensionType` enum ordering —
/// sequential, immediately after `MetadataPointer`=18, no explicit
/// discriminant overrides). Entry starts right after the mint's
/// self-referential `MetadataPointer` TLV entry, which always occupies
/// bytes `[166..234)` for this program's coins (165 base + 1 `AccountType`
/// marker + 4 header + 64 value). Returns `(update_authority_is_none, name,
/// symbol, uri)`, decoding the real Borsh `String`/`MaybeNull<Address>`
/// wire layout directly rather than pulling in the interface crate as a
/// test dependency.
fn read_token_metadata(data: &[u8]) -> (bool, String, String, String) {
    const TOKEN_METADATA_TLV_TYPE: u16 = 19;
    let entry_type = u16::from_le_bytes(data[234..236].try_into().unwrap());
    assert_eq!(entry_type, TOKEN_METADATA_TLV_TYPE, "expected TokenMetadata TLV entry right after MetadataPointer");
    let entry_len = u16::from_le_bytes(data[236..238].try_into().unwrap()) as usize;
    let value = &data[238..238 + entry_len];

    let update_authority_is_none = value[0..32] == [0u8; 32];
    let mut offset = 32 + 32; // update_authority + mint
    let mut read_string = || {
        let len = u32::from_le_bytes(value[offset..offset + 4].try_into().unwrap()) as usize;
        offset += 4;
        let s = std::str::from_utf8(&value[offset..offset + len]).unwrap().to_string();
        offset += len;
        s
    };
    let name = read_string();
    let symbol = read_string();
    let uri = read_string();
    (update_authority_is_none, name, symbol, uri)
}

#[test]
fn initialize_creates_global() {
    let provider = setup();
    let (global_pda, user) = init_global(&provider);

    let global = fetch_global(&provider, &global_pda).expect("global should be readable");
    assert!(bool::from(global.initialized));
    assert_eq!(global.authority, user.address());
}

#[test]
fn create_creates_bonding_curve_and_mint() {
    let provider = setup();
    let (global_pda, _user) = init_global(&provider);
    let creator = Keypair::new().address();

    let (bonding_curve_pda, mint, _bump) = create_bonding_curve(&provider, global_pda, creator);

    let bonding_curve = fetch_bonding_curve(&provider, &bonding_curve_pda)
        .expect("bonding_curve should be readable");
    assert_eq!(bonding_curve.creator, creator);

    assert_eq!(read_mint_decimals(&provider, &mint.address()), 6);
}

/// Real facts asserted here (confirmed against real deployed `pump.so`
/// bytecode via `reference/fee-tier-probe/src/bin/probe55.rs`, cross-checked
/// against 4 real mainnet `create_v2` transactions): a Token-2022 mint with
/// a self-referential `MetadataPointer` + real `TokenMetadata`, metadata
/// update authority revoked immediately after being set, mint authority
/// revoked after minting the full supply to `associated_bonding_curve`.
#[test]
fn create_v2_creates_token2022_mint_with_metadata_and_revokes_authorities() {
    let provider = setup();
    let (global_pda, user) = init_global(&provider);
    let creator = Keypair::new().address();

    build_toggle_create_v2(
        &provider,
        PROGRAM_ID,
        Bool::from(true),
        ToggleCreateV2Accounts { global: global_pda, authority: user.address() },
    )
    .signer(&user)
    .send_and_confirm()
    .expect("toggle_create_v2 should succeed");

    build_toggle_cashback_enabled(
        &provider,
        PROGRAM_ID,
        Bool::from(true),
        ToggleCashbackEnabledAccounts { global: global_pda, authority: user.address() },
    )
    .signer(&user)
    .send_and_confirm()
    .expect("toggle_cashback_enabled should succeed");

    let (bonding_curve_pda, mint, _bump) =
        create_bonding_curve_v2(&provider, global_pda, creator, Bool::from(true));

    let bonding_curve = fetch_bonding_curve(&provider, &bonding_curve_pda)
        .expect("bonding_curve should be readable");
    assert_eq!(bonding_curve.creator, creator);
    assert!(bool::from(bonding_curve.is_cashback_coin));
    assert!(!bool::from(bonding_curve.is_mayhem_mode));
    assert_eq!(bonding_curve.quote_mint, Address::default());

    let global = fetch_global(&provider, &global_pda).expect("global should be readable");

    let mint_data = provider.get_account_data(&mint.address()).expect("mint account should exist");
    assert_eq!(read_mint_decimals(&provider, &mint.address()), 6);
    // Mint authority `Option` presence flag (byte 0, `COption<Pubkey>` layout)
    // -- 0 means `None`, confirming `SetAuthority(MintTokens, None)` ran.
    assert_eq!(mint_data[0], 0, "mint authority should be revoked after create_v2");
    let supply = u64::from_le_bytes(mint_data[36..44].try_into().unwrap());
    assert_eq!(supply, global.token_total_supply);

    let (update_authority_is_none, name, symbol, uri) = read_token_metadata(&mint_data);
    assert!(update_authority_is_none, "TokenMetadata update authority should be revoked after create_v2");
    assert_eq!(name, "Test Coin V2");
    assert_eq!(symbol, "TESTV2");
    assert_eq!(uri, "https://example.com/metadata-v2.json");

    let (associated_bonding_curve_pda, _) = Address::find_program_address(
        &[bonding_curve_pda.as_ref(), TOKEN_2022_PROGRAM_ID.as_ref(), mint.address().as_ref()],
        &ASSOCIATED_TOKEN_PROGRAM_ID,
    );
    assert_eq!(token_balance(&provider, &associated_bonding_curve_pda), global.token_total_supply);
}

/// End-to-end proof that `add_quote_mint`/`remove_quote_mint` actually gate
/// a real instruction (`create_v2`), not just the raw `Global.whitelisted_quote_mints`
/// array: add a mint, confirm `create_v2` against it succeeds and the
/// resulting `bonding_curve` is genuinely paired with it, then remove it and
/// confirm a fresh `create_v2` against the same mint now fails.
#[test]
fn add_quote_mint_enables_and_remove_quote_mint_disables_create_v2_with_that_mint() {
    let provider = setup();
    let (global_pda, authority) = init_global(&provider);
    let creator = Keypair::new().address();

    build_toggle_create_v2(
        &provider,
        PROGRAM_ID,
        Bool::from(true),
        ToggleCreateV2Accounts { global: global_pda, authority: authority.address() },
    )
    .signer(&authority)
    .send_and_confirm()
    .expect("toggle_create_v2 should succeed");

    // Mimic-SOL: a freshly minted, 9-decimal classic SPL Token mint, not a
    // real cloned mainnet mint -- litesvm has no network access, and
    // `add_quote_mint`/`create_v2` only need a real, valid mint account, not
    // any specific one.
    let quote_mint = Keypair::new().address();
    provider
        .set_account(&quote_mint, spl_mint_account_data(None, 9), &TOKEN_PROGRAM_ID, 10_000_000)
        .expect("inject mimic-SOL quote mint fixture");

    build_add_quote_mint(
        &provider,
        PROGRAM_ID,
        quote_mint,
        AddQuoteMintAccounts { global: global_pda, authority: authority.address() },
    )
    .signer(&authority)
    .send_and_confirm()
    .expect("add_quote_mint should succeed");

    // Real value, not a guess: `Global.initial_virtual_quote_reserves` fetched
    // directly off the real mainnet `Global` account, confirmed via
    // `reference/fee-tier-probe/src/bin/probe64.rs`'s own printed output.
    const REAL_INITIAL_VIRTUAL_QUOTE_RESERVES: u64 = 4_292_000_000;
    build_set_virtual_quote_reserves(
        &provider,
        PROGRAM_ID,
        REAL_INITIAL_VIRTUAL_QUOTE_RESERVES,
        SetVirtualQuoteReservesAccounts { global: global_pda, authority: authority.address() },
    )
    .signer(&authority)
    .send_and_confirm()
    .expect("set_virtual_quote_reserves should succeed");

    let (result, bonding_curve_pda, _mint) =
        create_bonding_curve_v2_with_quote_mint(&provider, global_pda, creator, Some(quote_mint));
    result.expect("create_v2 with a whitelisted quote mint should succeed");

    let bonding_curve = fetch_bonding_curve(&provider, &bonding_curve_pda)
        .expect("bonding_curve should be readable");
    assert_eq!(bonding_curve.quote_mint, quote_mint, "bonding_curve should be paired with the whitelisted quote mint");
    assert_eq!(
        bonding_curve.virtual_quote_reserves, REAL_INITIAL_VIRTUAL_QUOTE_RESERVES,
        "non-SOL-paired curve should seed virtual_quote_reserves from Global.initial_virtual_quote_reserves"
    );

    build_remove_quote_mint(
        &provider,
        PROGRAM_ID,
        quote_mint,
        RemoveQuoteMintAccounts { global: global_pda, authority: authority.address() },
    )
    .signer(&authority)
    .send_and_confirm()
    .expect("remove_quote_mint should succeed");

    let (result, _bonding_curve_pda, _mint) =
        create_bonding_curve_v2_with_quote_mint(&provider, global_pda, creator, Some(quote_mint));
    // `PumpError::QuoteMintNotWhitelisted` is enum index 5 -> 6000 + 5 = 6005.
    assert_custom_code(result, 6005);
}

#[test]
fn set_params_updates_admin_set_creator_authority() {
    let provider = setup();
    let (global_pda, user) = init_global(&provider);

    // `set_params` requires exactly 8 `remaining_accounts`: `remaining_accounts[0]`
    // -> `global.fee_recipient`, `remaining_accounts[1..8]` -> `global.fee_recipients`,
    // each rent-exempt.
    let fee_recipient_accounts: Vec<Address> = (0..8)
        .map(|_| {
            let address = Keypair::new().address();
            provider.airdrop(&address, 10_000_000_000).unwrap();
            address
        })
        .collect();

    let new_admin_set_creator_authority = Keypair::new().address();
    build_set_params(
        &provider,
        PROGRAM_ID,
        pump_client::SetParamsArgs {
            initial_virtual_token_reserves: 0,
            initial_virtual_sol_reserves: 0,
            initial_real_token_reserves: 0,
            token_total_supply: 0,
            fee_basis_points: 0,
            withdraw_authority: Address::default(),
            enable_migrate: Bool::from(false),
            pool_migration_fee: 0,
            creator_fee_basis_points: 0,
            set_creator_authority: Address::default(),
            admin_set_creator_authority: new_admin_set_creator_authority,
        },
        SetParamsAccounts { global: global_pda, authority: user.address() },
    )
    .signer(&user)
    .remaining_accounts(
        fee_recipient_accounts
            .iter()
            .map(|address| AccountMeta { address: *address, is_signer: false, is_writable: false })
            .collect(),
    )
    .log()
    .send_and_confirm()
    .expect("set_params should succeed when signed by global.authority");

    let global = fetch_global(&provider, &global_pda).expect("global should be readable");
    assert_eq!(global.admin_set_creator_authority, new_admin_set_creator_authority);
}

#[test]
fn set_params_rejects_non_authority_signer() {
    let provider = setup();
    let (global_pda, _user) = init_global(&provider);

    let not_authority = Keypair::new();
    provider.airdrop(&not_authority.address(), 10_000_000_000).unwrap();

    let result = build_set_params(
        &provider,
        PROGRAM_ID,
        pump_client::SetParamsArgs {
            initial_virtual_token_reserves: 0,
            initial_virtual_sol_reserves: 0,
            initial_real_token_reserves: 0,
            token_total_supply: 0,
            fee_basis_points: 0,
            withdraw_authority: Address::default(),
            enable_migrate: Bool::from(false),
            pool_migration_fee: 0,
            creator_fee_basis_points: 0,
            set_creator_authority: Address::default(),
            admin_set_creator_authority: Keypair::new().address(),
        },
        SetParamsAccounts { global: global_pda, authority: not_authority.address() },
    )
    .signer(&not_authority)
    .log()
    .send_and_confirm();

    // `PumpError::NotAuthorized` is enum index 0 -> 6000 + 0 = 6000.
    assert_custom_code(result, 6000);
}

#[test]
fn set_reserved_fee_recipients_updates_global() {
    let provider = setup();
    let (global_pda, user) = init_global(&provider);

    // `set_reserved_fee_recipients` requires exactly 8 `remaining_accounts`:
    // `remaining_accounts[0]` -> `global.reserved_fee_recipient`,
    // `remaining_accounts[1..8]` -> `global.reserved_fee_recipients`, each
    // rent-exempt (confirmed via `reference/fee-tier-probe/src/bin/probe68.rs`
    // against real deployed `pump.so`).
    let recipient_accounts: Vec<Address> = (0..8)
        .map(|_| {
            let address = Keypair::new().address();
            provider.airdrop(&address, 10_000_000_000).unwrap();
            address
        })
        .collect();

    let whitelist_pda = Keypair::new().address();
    build_set_reserved_fee_recipients(
        &provider,
        PROGRAM_ID,
        whitelist_pda,
        SetReservedFeeRecipientsAccounts { global: global_pda, authority: user.address() },
    )
    .signer(&user)
    .remaining_accounts(
        recipient_accounts
            .iter()
            .map(|address| AccountMeta { address: *address, is_signer: false, is_writable: false })
            .collect(),
    )
    .log()
    .send_and_confirm()
    .expect("set_reserved_fee_recipients should succeed when signed by global.authority");

    let global = fetch_global(&provider, &global_pda).expect("global should be readable");
    assert_eq!(global.whitelist_pda, whitelist_pda);
    assert_eq!(global.reserved_fee_recipient, recipient_accounts[0]);
    assert_eq!(global.reserved_fee_recipients, recipient_accounts[1..8]);
}

#[test]
fn set_reserved_fee_recipients_rejects_non_authority_signer() {
    let provider = setup();
    let (global_pda, _user) = init_global(&provider);

    let not_authority = Keypair::new();
    provider.airdrop(&not_authority.address(), 10_000_000_000).unwrap();

    let recipient_accounts: Vec<Address> = (0..8)
        .map(|_| {
            let address = Keypair::new().address();
            provider.airdrop(&address, 10_000_000_000).unwrap();
            address
        })
        .collect();

    let result = build_set_reserved_fee_recipients(
        &provider,
        PROGRAM_ID,
        Keypair::new().address(),
        SetReservedFeeRecipientsAccounts { global: global_pda, authority: not_authority.address() },
    )
    .signer(&not_authority)
    .remaining_accounts(
        recipient_accounts
            .iter()
            .map(|address| AccountMeta { address: *address, is_signer: false, is_writable: false })
            .collect(),
    )
    .log()
    .send_and_confirm();

    // `PumpError::NotAuthorized` is enum index 0 -> 6000 + 0 = 6000.
    assert_custom_code(result, 6000);
}

#[test]
fn set_reserved_fee_recipients_rejects_wrong_remaining_accounts_count() {
    let provider = setup();
    let (global_pda, user) = init_global(&provider);

    let recipient_accounts: Vec<Address> = (0..3)
        .map(|_| {
            let address = Keypair::new().address();
            provider.airdrop(&address, 10_000_000_000).unwrap();
            address
        })
        .collect();

    let result = build_set_reserved_fee_recipients(
        &provider,
        PROGRAM_ID,
        Keypair::new().address(),
        SetReservedFeeRecipientsAccounts { global: global_pda, authority: user.address() },
    )
    .signer(&user)
    .remaining_accounts(
        recipient_accounts
            .iter()
            .map(|address| AccountMeta { address: *address, is_signer: false, is_writable: false })
            .collect(),
    )
    .log()
    .send_and_confirm();

    // `PumpError::NotEnoughRemainingAccounts` is enum index 1 -> 6000 + 1 = 6001.
    assert_custom_code(result, 6001);
}

struct MetaplexCreatorSpec {
    address: Address,
    verified: bool,
    share: u8,
}

/// Encodes a real Metaplex `Metadata` account's Borsh layout, confirmed
/// directly from the pinned `mpl-token-metadata` crate source
/// (`5.1.2-alpha.2`, matching `reference/pump-rust-client/Cargo.lock`) --
/// same layout `reference/fee-tier-probe/src/bin/probe69.rs` used to settle
/// `set_creator`/`set_metaplex_creator`'s real behavior. Everything after
/// `is_mutable` is `None` (a single `0u8` each); only `creators` is
/// plausibly relevant to either instruction.
fn metaplex_metadata_account_data(mint: &Address, update_authority: &Address, creators: &[MetaplexCreatorSpec]) -> Vec<u8> {
    let mut data = Vec::new();
    data.push(4); // Key::MetadataV1
    data.extend_from_slice(update_authority.as_ref());
    data.extend_from_slice(mint.as_ref());
    for s in ["Test Coin", "TEST", "https://example.com/metadata.json"] {
        data.extend_from_slice(&(s.len() as u32).to_le_bytes());
        data.extend_from_slice(s.as_bytes());
    }
    data.extend_from_slice(&0u16.to_le_bytes()); // seller_fee_basis_points
    if creators.is_empty() {
        data.push(0); // creators: None
    } else {
        data.push(1);
        data.extend_from_slice(&(creators.len() as u32).to_le_bytes());
        for c in creators {
            data.extend_from_slice(c.address.as_ref());
            data.push(c.verified as u8);
            data.push(c.share);
        }
    }
    data.push(0); // primary_sale_happened: false
    data.push(1); // is_mutable: true
    data.push(0); // edition_nonce: None
    data.push(0); // token_standard: None
    data.push(0); // collection: None
    data.push(0); // uses: None
    data.push(0); // collection_details: None
    data.push(0); // programmable_config: None
    data
}

/// Overwrites the real, `create`-CPI'd Metaplex metadata account for `mint`
/// (see `load_mpl_token_metadata_program`) with a synthetic one carrying the
/// given `creators` -- `create`'s own CPI (`metadata_cpi.rs`) always encodes
/// `creators: None`, so a real fixture created via `create_bonding_curve`
/// never has any on its own.
fn set_metaplex_metadata_creators(provider: &NaclacProvider, mint: &Address, creators: &[MetaplexCreatorSpec]) {
    let (metadata_pda, _) = get_metadata_pda(mint);
    let data = metaplex_metadata_account_data(mint, &Address::default(), creators);
    provider.set_account(&metadata_pda, data, &MPL_TOKEN_METADATA_PROGRAM_ID, 10_000_000).expect("set_account should succeed");
}

/// Sets `global.set_creator_authority` via `set_params`, leaving every other
/// field at its current on-chain value untouched except the 8 fee-recipient
/// `remaining_accounts` `set_params` unconditionally requires (rent-exempt,
/// otherwise unused by these tests).
fn set_global_set_creator_authority(provider: &NaclacProvider, global_pda: Address, authority: &Keypair, new_set_creator_authority: Address) {
    let fee_recipient_accounts: Vec<Address> = (0..8)
        .map(|_| {
            let address = Keypair::new().address();
            provider.airdrop(&address, 10_000_000_000).unwrap();
            address
        })
        .collect();

    build_set_params(
        provider,
        PROGRAM_ID,
        pump_client::SetParamsArgs {
            initial_virtual_token_reserves: 0,
            initial_virtual_sol_reserves: 0,
            initial_real_token_reserves: 0,
            token_total_supply: 0,
            fee_basis_points: 0,
            withdraw_authority: Address::default(),
            enable_migrate: Bool::from(false),
            pool_migration_fee: 0,
            creator_fee_basis_points: 0,
            set_creator_authority: new_set_creator_authority,
            admin_set_creator_authority: Address::default(),
        },
        SetParamsAccounts { global: global_pda, authority: authority.address() },
    )
    .signer(authority)
    .remaining_accounts(
        fee_recipient_accounts
            .iter()
            .map(|address| AccountMeta { address: *address, is_signer: false, is_writable: false })
            .collect(),
    )
    .log()
    .send_and_confirm()
    .expect("set_params should succeed");
}

#[test]
fn set_metaplex_creator_syncs_first_creator_from_real_metadata() {
    let provider = setup();
    let (global_pda, _user) = init_global(&provider);
    let (bonding_curve_pda, mint, bonding_curve_bump) = create_bonding_curve(&provider, global_pda, Address::default());
    let (metadata_pda, metadata_bump) = get_metadata_pda(&mint.address());

    let unverified = Keypair::new().address();
    let verified = Keypair::new().address();
    // Confirmed via `probe69.rs`: real pump.so picks `creators[0]` literally,
    // not "whichever entry is verified" -- `unverified` here is `creators[0]`.
    set_metaplex_metadata_creators(&provider, &mint.address(), &[
        MetaplexCreatorSpec { address: unverified, verified: false, share: 50 },
        MetaplexCreatorSpec { address: verified, verified: true, share: 50 },
    ]);

    build_set_metaplex_creator(
        &provider,
        PROGRAM_ID,
        metadata_bump,
        bonding_curve_bump,
        SetMetaplexCreatorAccounts { mint: mint.address(), metadata: metadata_pda, bonding_curve: bonding_curve_pda },
    )
    .log()
    .send_and_confirm()
    .expect("set_metaplex_creator should succeed");

    let bonding_curve = fetch_bonding_curve(&provider, &bonding_curve_pda).expect("bonding_curve should be readable");
    assert_eq!(bonding_curve.creator, unverified);
}

#[test]
fn set_metaplex_creator_noops_when_creators_missing() {
    let provider = setup();
    let (global_pda, _user) = init_global(&provider);
    // `create`'s own metadata CPI always encodes `creators: None` -- no
    // patching needed to test the "nothing usable" path.
    let (bonding_curve_pda, mint, bonding_curve_bump) = create_bonding_curve(&provider, global_pda, Address::default());
    let (metadata_pda, metadata_bump) = get_metadata_pda(&mint.address());

    build_set_metaplex_creator(
        &provider,
        PROGRAM_ID,
        metadata_bump,
        bonding_curve_bump,
        SetMetaplexCreatorAccounts { mint: mint.address(), metadata: metadata_pda, bonding_curve: bonding_curve_pda },
    )
    .log()
    .send_and_confirm()
    .expect("set_metaplex_creator should succeed as a no-op");

    let bonding_curve = fetch_bonding_curve(&provider, &bonding_curve_pda).expect("bonding_curve should be readable");
    assert_eq!(bonding_curve.creator, Address::default());
}

#[test]
fn set_metaplex_creator_noops_when_already_set() {
    let provider = setup();
    let (global_pda, _user) = init_global(&provider);
    let original_creator = Keypair::new().address();
    let (bonding_curve_pda, mint, bonding_curve_bump) = create_bonding_curve(&provider, global_pda, original_creator);
    let (metadata_pda, metadata_bump) = get_metadata_pda(&mint.address());
    set_metaplex_metadata_creators(&provider, &mint.address(), &[
        MetaplexCreatorSpec { address: Keypair::new().address(), verified: true, share: 100 },
    ]);

    build_set_metaplex_creator(
        &provider,
        PROGRAM_ID,
        metadata_bump,
        bonding_curve_bump,
        SetMetaplexCreatorAccounts { mint: mint.address(), metadata: metadata_pda, bonding_curve: bonding_curve_pda },
    )
    .log()
    .send_and_confirm()
    .expect("set_metaplex_creator should succeed as a no-op");

    let bonding_curve = fetch_bonding_curve(&provider, &bonding_curve_pda).expect("bonding_curve should be readable");
    assert_eq!(bonding_curve.creator, original_creator);
}

#[test]
fn set_creator_prefers_metadata_creator_over_arg() {
    let provider = setup();
    let (global_pda, user) = init_global(&provider);
    let set_creator_authority = Keypair::new();
    provider.airdrop(&set_creator_authority.address(), 10_000_000_000).unwrap();
    set_global_set_creator_authority(&provider, global_pda, &user, set_creator_authority.address());

    let (bonding_curve_pda, mint, bonding_curve_bump) = create_bonding_curve(&provider, global_pda, Address::default());
    let (metadata_pda, metadata_bump) = get_metadata_pda(&mint.address());
    let metadata_creator = Keypair::new().address();
    set_metaplex_metadata_creators(&provider, &mint.address(), &[
        MetaplexCreatorSpec { address: metadata_creator, verified: true, share: 100 },
    ]);
    let arg_creator = Keypair::new().address();

    build_set_creator(
        &provider,
        PROGRAM_ID,
        arg_creator,
        metadata_bump,
        bonding_curve_bump,
        SetCreatorAccounts {
            set_creator_authority: set_creator_authority.address(),
            global: global_pda,
            mint: mint.address(),
            metadata: metadata_pda,
            bonding_curve: bonding_curve_pda,
        },
    )
    .signer(&set_creator_authority)
    .log()
    .send_and_confirm()
    .expect("set_creator should succeed");

    // Confirmed via `probe69.rs`: metadata's `creators[0]` wins outright when
    // present -- the `creator` arg is ignored, not used as a fallback.
    let bonding_curve = fetch_bonding_curve(&provider, &bonding_curve_pda).expect("bonding_curve should be readable");
    assert_eq!(bonding_curve.creator, metadata_creator);
}

#[test]
fn set_creator_falls_back_to_arg_when_metadata_has_no_creators() {
    let provider = setup();
    let (global_pda, user) = init_global(&provider);
    let set_creator_authority = Keypair::new();
    provider.airdrop(&set_creator_authority.address(), 10_000_000_000).unwrap();
    set_global_set_creator_authority(&provider, global_pda, &user, set_creator_authority.address());

    // `create`'s own metadata CPI always encodes `creators: None`.
    let (bonding_curve_pda, mint, bonding_curve_bump) = create_bonding_curve(&provider, global_pda, Address::default());
    let (metadata_pda, metadata_bump) = get_metadata_pda(&mint.address());
    let arg_creator = Keypair::new().address();

    build_set_creator(
        &provider,
        PROGRAM_ID,
        arg_creator,
        metadata_bump,
        bonding_curve_bump,
        SetCreatorAccounts {
            set_creator_authority: set_creator_authority.address(),
            global: global_pda,
            mint: mint.address(),
            metadata: metadata_pda,
            bonding_curve: bonding_curve_pda,
        },
    )
    .signer(&set_creator_authority)
    .log()
    .send_and_confirm()
    .expect("set_creator should succeed");

    let bonding_curve = fetch_bonding_curve(&provider, &bonding_curve_pda).expect("bonding_curve should be readable");
    assert_eq!(bonding_curve.creator, arg_creator);
}

#[test]
fn set_creator_rejects_non_authority_signer() {
    let provider = setup();
    let (global_pda, user) = init_global(&provider);
    let set_creator_authority = Keypair::new();
    provider.airdrop(&set_creator_authority.address(), 10_000_000_000).unwrap();
    set_global_set_creator_authority(&provider, global_pda, &user, set_creator_authority.address());

    let not_authority = Keypair::new();
    provider.airdrop(&not_authority.address(), 10_000_000_000).unwrap();

    let (bonding_curve_pda, mint, bonding_curve_bump) = create_bonding_curve(&provider, global_pda, Address::default());
    let (metadata_pda, metadata_bump) = get_metadata_pda(&mint.address());

    let result = build_set_creator(
        &provider,
        PROGRAM_ID,
        Keypair::new().address(),
        metadata_bump,
        bonding_curve_bump,
        SetCreatorAccounts {
            set_creator_authority: not_authority.address(),
            global: global_pda,
            mint: mint.address(),
            metadata: metadata_pda,
            bonding_curve: bonding_curve_pda,
        },
    )
    .signer(&not_authority)
    .log()
    .send_and_confirm();

    // `PumpError::NotAuthorized` is enum index 0 -> 6000 + 0 = 6000.
    assert_custom_code(result, 6000);
}

/// `update_buyback_config` itself accepts any bare address as a recipient,
/// no owner/seeds check at all (confirmed via `probe65.rs` against real
/// deployed `pump.so`) -- the real enforcement is later, at trade time:
/// `buy_v2`/`sell_v2`'s own `seeds = [BUYBACK_VAULT_SEED, &[index]]`
/// constraint on `buyback_fee_recipient` requires the real PDA to genuinely
/// exist and be owned by `PUMP_FEES_PROGRAM_ID`. A bare fixture account at
/// the right PDA address is sufficient for that, without needing to load
/// and run the real `pump_fees.so`.
fn inject_buyback_vault_fixtures(provider: &NaclacProvider) -> ([Address; 8], [u8; 8]) {
    let mut addresses = [Address::default(); 8];
    let mut bumps = [0u8; 8];
    for i in 0..8u8 {
        let (pda, bump) = Address::find_program_address(
            &[BUYBACK_VAULT_SEED, &[i]],
            &PUMP_FEES_PROGRAM_ID,
        );
        provider
            .set_account(&pda, vec![], &PUMP_FEES_PROGRAM_ID, 890_880)
            .expect("inject buyback_vault fixture");
        addresses[i as usize] = pda;
        bumps[i as usize] = bump;
    }
    (addresses, bumps)
}

/// Real discriminator (name-hash based, same value already confirmed in
/// `reference/fee-tier-probe/src/bin/probe15.rs`/`probe16.rs`), zero-filled
/// otherwise — `buy_v2` only reads this account's owner/discriminator, never
/// its field values, for a coin that hasn't opted into volume tracking.
fn global_volume_accumulator_account_data() -> Vec<u8> {
    let mut data = vec![202, 42, 246, 43, 142, 190, 30, 255];
    data.extend_from_slice(&[0u8; 592]); // 600 bytes total, matches the real account size
    data
}

#[test]
fn update_buyback_config_updates_global() {
    let provider = setup();
    let (global_pda, user) = init_global(&provider);
    let (vaults, bumps) = inject_buyback_vault_fixtures(&provider);

    build_update_buyback_config(
        &provider,
        PROGRAM_ID,
        Some(1234u64),
        bumps,
        UpdateBuybackConfigAccounts { global: global_pda, authority: user.address() },
    )
    .signer(&user)
    .remaining_accounts(
        vaults.iter().map(|address| AccountMeta { address: *address, is_signer: false, is_writable: false }).collect(),
    )
    .log()
    .send_and_confirm()
    .expect("update_buyback_config should succeed when signed by global.authority");

    let global = fetch_global(&provider, &global_pda).expect("global should be readable");
    assert_eq!(global.buyback_fee_recipients, vaults);
    assert_eq!(global.buyback_basis_points, 1234);
}

#[test]
fn update_buyback_config_rejects_non_authority_signer() {
    let provider = setup();
    let (global_pda, _user) = init_global(&provider);
    let (vaults, bumps) = inject_buyback_vault_fixtures(&provider);

    let not_authority = Keypair::new();
    provider.airdrop(&not_authority.address(), 10_000_000_000).unwrap();

    let result = build_update_buyback_config(
        &provider,
        PROGRAM_ID,
        Some(1234u64),
        bumps,
        UpdateBuybackConfigAccounts { global: global_pda, authority: not_authority.address() },
    )
    .signer(&not_authority)
    .remaining_accounts(
        vaults.iter().map(|address| AccountMeta { address: *address, is_signer: false, is_writable: false }).collect(),
    )
    .log()
    .send_and_confirm();

    // `PumpError::NotAuthorized` is enum index 0 -> 6000 + 0 = 6000.
    assert_custom_code(result, 6000);
}

/// Real, confirmed behavior (`probe65.rs` against real deployed `pump.so`,
/// not inferred): remaining_accounts must be exactly 0 or 8 -- any other
/// count is rejected with the real `WrongBuybackFeeRecipientsCount` error.
#[test]
fn update_buyback_config_rejects_wrong_remaining_accounts_count() {
    let provider = setup();
    let (global_pda, user) = init_global(&provider);
    let (vaults, bumps) = inject_buyback_vault_fixtures(&provider);

    let result = build_update_buyback_config(
        &provider,
        PROGRAM_ID,
        Some(1u64),
        bumps,
        UpdateBuybackConfigAccounts { global: global_pda, authority: user.address() },
    )
    .signer(&user)
    .remaining_accounts(
        vaults[0..3].iter().map(|address| AccountMeta { address: *address, is_signer: false, is_writable: false }).collect(),
    )
    .log()
    .send_and_confirm();

    // `PumpError::WrongBuybackFeeRecipientsCount` is the 29th enum variant
    // (index 28) -> 6000 + 28 = 6028 (see `errors.rs`'s declaration order).
    assert_custom_code(result, 6028);
}

/// Real, confirmed behavior (`probe65.rs`): `buyback_basis_points: None`
/// leaves `Global.buyback_basis_points` unchanged, it does not reset it to 0.
#[test]
fn update_buyback_config_none_leaves_basis_points_unchanged() {
    let provider = setup();
    let (global_pda, user) = init_global(&provider);

    build_update_buyback_config(
        &provider,
        PROGRAM_ID,
        Some(4321u64),
        [0u8; 8],
        UpdateBuybackConfigAccounts { global: global_pda, authority: user.address() },
    )
    .signer(&user)
    .log()
    .send_and_confirm()
    .expect("update_buyback_config with Some should succeed");

    build_update_buyback_config(
        &provider,
        PROGRAM_ID,
        None,
        [0u8; 8],
        UpdateBuybackConfigAccounts { global: global_pda, authority: user.address() },
    )
    .signer(&user)
    .log()
    .send_and_confirm()
    .expect("update_buyback_config with None should succeed");

    let global = fetch_global(&provider, &global_pda).expect("global should be readable");
    assert_eq!(global.buyback_basis_points, 4321, "None should leave buyback_basis_points unchanged");
}

fn load_pump_fees_program(provider: &NaclacProvider) {
    let mut pump_fees_root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    pump_fees_root.pop(); // programs
    pump_fees_root.pop(); // pump-bonding-curve workspace root
    pump_fees_root.pop(); // pump-fun-program
    pump_fees_root.push("pump-fees");
    let so_path = resolve_cargo_target_dir(&pump_fees_root).join("deploy/pump_fees.so");

    provider
        .add_program(&pump_fees_client::PROGRAM_ID, so_path.to_str().unwrap())
        .expect("Failed to load pump_fees.so");
}

fn load_pump_amm_program(provider: &NaclacProvider) {
    let mut pump_amm_root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    pump_amm_root.pop(); // programs
    pump_amm_root.pop(); // pump-bonding-curve workspace root
    pump_amm_root.pop(); // pump-fun-program
    pump_amm_root.push("pump-amm");
    let so_path = resolve_cargo_target_dir(&pump_amm_root).join("deploy/pump_amm.so");

    provider
        .add_program(&PUMP_AMM_PROGRAM_ID, so_path.to_str().unwrap())
        .expect("Failed to load pump_amm.so");
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

/// `pump_amm`'s own `GlobalConfig` component is a scoped mirror (`bump`,
/// `disable_flags`, `boost_enabled`, `admin`, `boost_authority`, not the
/// full real-mainnet layout) — matches `pump_amm_test.rs`'s own helper of
/// the same name/shape exactly.
fn global_config_account_data(disable_flags: u8) -> Vec<u8> {
    let mut data = vec![149, 8, 156, 202, 160, 252, 176, 217];
    data.push(0); // bump — unused by `create_pool`, value doesn't matter here
    data.push(disable_flags);
    data.push(0); // boost_enabled — unused by `create_pool`/classic `migrate`
    data.extend_from_slice(&[0u8; 32]); // admin
    data.extend_from_slice(&[0u8; 32]); // boost_authority
    data
}

fn discriminated_bytes<T: bytemuck::Pod>(disc: [u8; 8], value: &T) -> Vec<u8> {
    let mut out = Vec::with_capacity(8 + core::mem::size_of::<T>());
    out.extend_from_slice(&disc);
    out.extend_from_slice(bytemuck::bytes_of(value));
    out
}

/// Injects a `pump_fees::FeeConfig` for `config_program_id = PROGRAM_ID`
/// (`pump`'s own self-reference, matching `buy`/`sell`'s `fee_config` seeds)
/// directly, bypassing `pump_fees::initialize_fee_config` — mirrors
/// `pump_fees_test.rs`'s helper of the same shape. `buy`/`sell` genuinely CPI
/// into `pump_fees::get_fees`, which reads this account for real, and
/// requires at least one `fee_tiers` entry with a zero threshold for the
/// `is_pump_pool = true` / `market_cap_lamports = 0` path they always use.
fn setup_fee_config(provider: &NaclacProvider, admin: Address, fee_tiers: Vec<pump_fees_client::FeeTier>) -> Address {
    let (pda, bump) = Address::find_program_address(
        &[FEE_CONFIG_SEED, PROGRAM_ID.as_ref()],
        &PUMP_FEES_PROGRAM_ID,
    );
    let zero_tier = pump_fees_client::FeeTier {
        market_cap_lamports_threshold: 0,
        fees: pump_fees_client::Fees { lp_fee_bps: 0, protocol_fee_bps: 0, creator_fee_bps: 0 },
    };
    let mut tiers_arr = [zero_tier; 50];
    for (i, t) in fee_tiers.iter().enumerate() {
        tiers_arr[i] = *t;
    }
    // `get_fees` reads `stable_fee_tiers` instead of `fee_tiers` whenever
    // `is_new_quote_mint` is true (any non-SOL quote mint, `fees-04-fee-math-
    // and-formulas.md` #5, confirmed against real bytecode via `probe5.rs`).
    // Deliberately given *different* bps than `fee_tiers` (not mirrored) --
    // `TEST_STABLE_PROTOCOL_FEE_BPS`/`TEST_STABLE_CREATOR_FEE_BPS` below --
    // so a test asserting against these values actually proves the correct
    // table was read, rather than passing identically regardless of which
    // table `is_new_quote_mint` picked.
    let stable_tier = pump_fees_client::FeeTier {
        market_cap_lamports_threshold: 0,
        fees: pump_fees_client::Fees {
            lp_fee_bps: 0,
            protocol_fee_bps: TEST_STABLE_PROTOCOL_FEE_BPS,
            creator_fee_bps: TEST_STABLE_CREATOR_FEE_BPS,
        },
    };
    let mut stable_tiers_arr = [zero_tier; 50];
    stable_tiers_arr[0] = stable_tier;
    let cfg = pump_fees_client::FeeConfig {
        flat_fees: zero_tier.fees,
        fee_tiers: tiers_arr,
        stable_fee_tiers: stable_tiers_arr,
        fee_tiers_len: fee_tiers.len() as u32,
        stable_fee_tiers_len: 1,
        bump,
        admin,
    };
    let data = discriminated_bytes(pump_fees_client::FEECONFIG_DISCRIMINATOR, &cfg);
    provider
        .set_account(&pda, data, &PUMP_FEES_PROGRAM_ID, 10_000_000_000)
        .expect("inject fee_config fixture");
    pda
}

/// Real end-to-end setup for `buy`/`sell`: loads the real `pump_fees.so`
/// alongside `pump` (they genuinely CPI into `pump_fees::get_fees`),
/// initializes `global` with pump-fun's real default bonding-curve reserves
/// (`reference/pump-rust-client`'s `math/bonding_curve.rs` constants),
/// registers 8 fee-recipient accounts and 8 buyback vaults, creates a
/// bonding curve + mint, and injects a real `pump_fees::FeeConfig` with one
/// zero-threshold fee tier.
#[allow(clippy::type_complexity)]
fn setup_tradeable_bonding_curve(
    provider: &NaclacProvider,
) -> (Address, Address, Keypair, u8, Address, [Address; 8], [Address; 8], [u8; 8], Keypair) {
    load_pump_fees_program(provider);

    let (global_pda, authority) = init_global(provider);

    let fee_recipient_accounts: [Address; 8] = std::array::from_fn(|_| {
        let address = Keypair::new().address();
        provider.airdrop(&address, 10_000_000_000).unwrap();
        address
    });

    build_set_params(
        provider,
        PROGRAM_ID,
        pump_client::SetParamsArgs {
            initial_virtual_token_reserves: 1_073_000_000_000_000,
            initial_virtual_sol_reserves: 30_000_000_000,
            initial_real_token_reserves: 793_100_000_000_000,
            token_total_supply: 1_000_000_000_000_000,
            fee_basis_points: 0,
            withdraw_authority: Address::default(),
            enable_migrate: Bool::from(false),
            pool_migration_fee: 0,
            creator_fee_basis_points: 0,
            set_creator_authority: Address::default(),
            admin_set_creator_authority: Address::default(),
        },
        SetParamsAccounts { global: global_pda, authority: authority.address() },
    )
    .signer(&authority)
    .remaining_accounts(
        fee_recipient_accounts
            .iter()
            .map(|address| AccountMeta { address: *address, is_signer: false, is_writable: false })
            .collect(),
    )
    .send_and_confirm()
    .expect("set_params should succeed");

    build_toggle_create_v2(
        provider,
        PROGRAM_ID,
        Bool::from(true),
        ToggleCreateV2Accounts { global: global_pda, authority: authority.address() },
    )
    .signer(&authority)
    .send_and_confirm()
    .expect("toggle_create_v2 should succeed");

    let (buyback_vaults, buyback_bumps) = inject_buyback_vault_fixtures(provider);

    build_update_buyback_config(
        provider,
        PROGRAM_ID,
        None,
        buyback_bumps,
        UpdateBuybackConfigAccounts { global: global_pda, authority: authority.address() },
    )
    .signer(&authority)
    .remaining_accounts(
        buyback_vaults
            .iter()
            .map(|address| AccountMeta { address: *address, is_signer: false, is_writable: false })
            .collect(),
    )
    .send_and_confirm()
    .expect("update_buyback_config should succeed");

    let creator = Keypair::new().address();
    let (bonding_curve_pda, mint, bonding_curve_bump) =
        create_bonding_curve(provider, global_pda, creator);

    setup_fee_config(
        provider,
        authority.address(),
        vec![pump_fees_client::FeeTier {
            market_cap_lamports_threshold: 0,
            fees: pump_fees_client::Fees { lp_fee_bps: 0, protocol_fee_bps: 100, creator_fee_bps: 50 },
        }],
    );

    (
        global_pda,
        bonding_curve_pda,
        mint,
        bonding_curve_bump,
        creator,
        fee_recipient_accounts,
        buyback_vaults,
        buyback_bumps,
        authority,
    )
}

#[test]
fn buy_purchases_tokens_and_updates_reserves() {
    let provider = setup();
    let (
        global_pda,
        bonding_curve_pda,
        mint,
        bonding_curve_bump,
        creator,
        fee_recipient_accounts,
        buyback_vaults,
        buyback_bumps,
        _authority,
    ) = setup_tradeable_bonding_curve(&provider);

    let user = Keypair::new();
    provider.airdrop(&user.address(), 10_000_000_000).unwrap();
    create_ata(&provider, &mint.address(), &user.address()).expect("create user ATA");

    let (associated_bonding_curve_pda, associated_bonding_curve_bump) = Address::find_program_address(
        &[bonding_curve_pda.as_ref(), TOKEN_PROGRAM_ID.as_ref(), mint.address().as_ref()],
        &ASSOCIATED_TOKEN_PROGRAM_ID,
    );
    let (associated_user_pda, associated_user_bump) = Address::find_program_address(
        &[user.address().as_ref(), TOKEN_PROGRAM_ID.as_ref(), mint.address().as_ref()],
        &ASSOCIATED_TOKEN_PROGRAM_ID,
    );
    let (creator_vault_pda, creator_vault_bump) = Address::find_program_address(
        &[CREATOR_VAULT_SEED, creator.as_ref()],
        &PROGRAM_ID,
    );
    let (global_volume_accumulator_pda, _) =
        Address::find_program_address(&[GLOBAL_VOLUME_ACCUMULATOR_SEED], &PROGRAM_ID);
    let (user_volume_accumulator_pda, user_volume_accumulator_bump) = Address::find_program_address(
        &[USER_VOLUME_ACCUMULATOR_SEED, user.address().as_ref()],
        &PROGRAM_ID,
    );
    let (fee_config_pda, fee_config_bump) = Address::find_program_address(
        &[FEE_CONFIG_SEED, PROGRAM_ID.as_ref()],
        &PUMP_FEES_PROGRAM_ID,
    );
    let (pump_authority_pda, _) = Address::find_program_address(&[PUMP_AUTHORITY_SEED], &PROGRAM_ID);
    let (bonding_curve_v2_pda_addr, bonding_curve_v2_bump) = bonding_curve_v2_pda(&mint.address());

    let bonding_curve_before = fetch_bonding_curve(&provider, &bonding_curve_pda)
        .expect("bonding_curve should be readable");

    // Large enough that `creator_fee` (0.5% of `net_sol`, per the injected
    // `FeeConfig`) clears the ~890_880 lamport rent-exempt minimum for the
    // brand-new, zero-lamport `creator_vault` PDA it gets paid into — a
    // smaller trade leaves `creator_vault` with a nonzero-but-below-rent-exempt
    // balance, which the runtime rejects outright.
    let amount = 15_000_000_000_000u64;

    let expected_net_sol = expected_gross_buy(
        amount as u128,
        bonding_curve_before.virtual_quote_reserves as u128,
        bonding_curve_before.virtual_token_reserves as u128,
    );
    let expected_protocol_fee = expected_fee_ceil(expected_net_sol as u128, TEST_PROTOCOL_FEE_BPS);
    let expected_creator_fee = expected_fee_ceil(expected_net_sol as u128, TEST_CREATOR_FEE_BPS);
    let expected_buyback_share = expected_fee_floor(expected_protocol_fee as u128, TEST_BUYBACK_BASIS_POINTS);
    let expected_fee_recipient_share = expected_protocol_fee - expected_buyback_share;
    let expected_total_cost = expected_net_sol + expected_protocol_fee + expected_creator_fee;

    let bonding_curve_lamports_before = provider.get_balance(&bonding_curve_pda).unwrap();
    let fee_recipient_lamports_before = provider.get_balance(&fee_recipient_accounts[0]).unwrap();
    let buyback_lamports_before = provider.get_balance(&buyback_vaults[0]).unwrap();
    let user_sol_before = provider.get_balance(&user.address()).unwrap();

    build_buy(
        &provider,
        PROGRAM_ID,
        pump_client::BuyArgs {
            amount,
            max_sol_cost: 1_000_000_000,
            track_volume: Bool::from(false),
            bonding_curve_bump,
            associated_bonding_curve_bump,
            associated_user_bump,
            creator_vault_bump,
            user_volume_accumulator_bump,
            fee_config_bump,
            buyback_index: 0,
            buyback_vault_bump: buyback_bumps[0],
            bonding_curve_v2_bump,
        },
        BuyAccounts {
            global: global_pda,
            fee_recipient: fee_recipient_accounts[0],
            mint: mint.address(),
            bonding_curve: bonding_curve_pda,
            user: user.address(),
            associated_bonding_curve: associated_bonding_curve_pda,
            associated_user: associated_user_pda,
            system_program: SYSTEM_PROGRAM_ID,
            token_program: TOKEN_PROGRAM_ID,
            creator_vault: creator_vault_pda,
            program: PROGRAM_ID,
            global_volume_accumulator: global_volume_accumulator_pda,
            user_volume_accumulator: user_volume_accumulator_pda,
            fee_config: fee_config_pda,
            fee_program: PUMP_FEES_PROGRAM_ID,
            bonding_curve_v2: bonding_curve_v2_pda_addr,
            buyback_fee_recipient: buyback_vaults[0],
            pump_authority: pump_authority_pda,
        },
    )
    .signer(&user)
    .log()
    .send_and_confirm()
    .expect("buy should succeed");

    let bonding_curve_after = fetch_bonding_curve(&provider, &bonding_curve_pda)
        .expect("bonding_curve should be readable");
    assert_eq!(
        bonding_curve_after.real_token_reserves,
        bonding_curve_before.real_token_reserves - amount,
        "buy should debit real_token_reserves by the purchased amount"
    );
    assert_eq!(
        bonding_curve_after.real_quote_reserves,
        bonding_curve_before.real_quote_reserves + expected_net_sol,
        "buy should credit real_quote_reserves by exactly the formula's net_sol"
    );
    assert_eq!(
        bonding_curve_after.virtual_quote_reserves,
        bonding_curve_before.virtual_quote_reserves + expected_net_sol,
        "buy should credit virtual_quote_reserves by exactly the formula's net_sol"
    );

    let ata_data = provider
        .get_account_data(&associated_user_pda)
        .expect("user's associated token account should exist");
    let balance = u64::from_le_bytes(ata_data[64..72].try_into().unwrap());
    assert_eq!(balance, amount, "user should have received the purchased tokens");

    let bonding_curve_lamports_after = provider.get_balance(&bonding_curve_pda).unwrap();
    assert_eq!(
        bonding_curve_lamports_after,
        bonding_curve_lamports_before + expected_net_sol,
        "bonding_curve's native lamport balance should be credited exactly net_sol"
    );
    let fee_recipient_lamports_after = provider.get_balance(&fee_recipient_accounts[0]).unwrap();
    assert_eq!(
        fee_recipient_lamports_after,
        fee_recipient_lamports_before + expected_fee_recipient_share,
        "fee_recipient should receive exactly protocol_fee minus the buyback carve-out"
    );
    let buyback_lamports_after = provider.get_balance(&buyback_vaults[0]).unwrap();
    assert_eq!(
        buyback_lamports_after, buyback_lamports_before + expected_buyback_share,
        "buyback_fee_recipient should receive exactly the buyback carve-out (0 in this fixture)"
    );
    // `creator_vault` starts completely empty (this is the very first trade
    // on this curve), so the confirmed conditional rent-exempt top-up fires:
    // exactly `CREATOR_VAULT_RENT_EXEMPT_MINIMUM` lamports, on top of
    // `creator_fee` itself (see `buy.rs`'s module comment for how this exact
    // formula was confirmed via probe52.rs).
    let creator_vault_lamports_after = provider.get_balance(&creator_vault_pda).unwrap();
    assert_eq!(
        creator_vault_lamports_after,
        CREATOR_VAULT_RENT_EXEMPT_MINIMUM + expected_creator_fee,
        "creator_vault should receive the rent-exempt top-up plus exactly creator_fee"
    );
    // This is the very first `buy` in this test, so `global_volume_accumulator`
    // and `user_volume_accumulator` are both created fresh here via
    // `init_if_needed`, payer = user — real, legitimate one-time rent, but
    // no part of the swap formula itself. Read the exact amount paid
    // directly from the resulting account balances rather than
    // hand-computing Solana's rent-exempt-minimum formula.
    let rent_paid_for_new_accounts = provider.get_balance(&global_volume_accumulator_pda).unwrap()
        + provider.get_balance(&user_volume_accumulator_pda).unwrap();
    let user_sol_after = provider.get_balance(&user.address()).unwrap();
    assert_eq!(
        user_sol_before - user_sol_after,
        expected_total_cost + CREATOR_VAULT_RENT_EXEMPT_MINIMUM + rent_paid_for_new_accounts,
        "user should pay exactly net_sol + protocol_fee + creator_fee, plus creator_vault's rent-exempt top-up, plus the one-time rent for newly-created accounts (the tx fee payer is a separate wallet, not `user`)"
    );
}

#[test]
fn sell_returns_tokens_and_updates_reserves() {
    let provider = setup();
    let (
        global_pda,
        bonding_curve_pda,
        mint,
        bonding_curve_bump,
        creator,
        fee_recipient_accounts,
        buyback_vaults,
        buyback_bumps,
        _authority,
    ) = setup_tradeable_bonding_curve(&provider);

    let user = Keypair::new();
    provider.airdrop(&user.address(), 10_000_000_000).unwrap();
    create_ata(&provider, &mint.address(), &user.address()).expect("create user ATA");

    let (associated_bonding_curve_pda, associated_bonding_curve_bump) = Address::find_program_address(
        &[bonding_curve_pda.as_ref(), TOKEN_PROGRAM_ID.as_ref(), mint.address().as_ref()],
        &ASSOCIATED_TOKEN_PROGRAM_ID,
    );
    let (associated_user_pda, associated_user_bump) = Address::find_program_address(
        &[user.address().as_ref(), TOKEN_PROGRAM_ID.as_ref(), mint.address().as_ref()],
        &ASSOCIATED_TOKEN_PROGRAM_ID,
    );
    let (creator_vault_pda, creator_vault_bump) = Address::find_program_address(
        &[CREATOR_VAULT_SEED, creator.as_ref()],
        &PROGRAM_ID,
    );
    let (global_volume_accumulator_pda, _) =
        Address::find_program_address(&[GLOBAL_VOLUME_ACCUMULATOR_SEED], &PROGRAM_ID);
    let (user_volume_accumulator_pda, user_volume_accumulator_bump) = Address::find_program_address(
        &[USER_VOLUME_ACCUMULATOR_SEED, user.address().as_ref()],
        &PROGRAM_ID,
    );
    let (fee_config_pda, fee_config_bump) = Address::find_program_address(
        &[FEE_CONFIG_SEED, PROGRAM_ID.as_ref()],
        &PUMP_FEES_PROGRAM_ID,
    );
    let (pump_authority_pda, _) = Address::find_program_address(&[PUMP_AUTHORITY_SEED], &PROGRAM_ID);
    let (bonding_curve_v2_pda_addr, bonding_curve_v2_bump) = bonding_curve_v2_pda(&mint.address());

    // Buy first, both to give `user` real tokens to sell back and to leave
    // `creator_vault` already rent-exempt (see `buy_purchases_tokens_and_updates_reserves`'s
    // comment) before `sell` adds more lamports to it.
    let buy_amount = 15_000_000_000_000u64;
    build_buy(
        &provider,
        PROGRAM_ID,
        pump_client::BuyArgs {
            amount: buy_amount,
            max_sol_cost: 1_000_000_000,
            track_volume: Bool::from(false),
            bonding_curve_bump,
            associated_bonding_curve_bump,
            associated_user_bump,
            creator_vault_bump,
            user_volume_accumulator_bump,
            fee_config_bump,
            buyback_index: 0,
            buyback_vault_bump: buyback_bumps[0],
            bonding_curve_v2_bump,
        },
        BuyAccounts {
            global: global_pda,
            fee_recipient: fee_recipient_accounts[0],
            mint: mint.address(),
            bonding_curve: bonding_curve_pda,
            user: user.address(),
            associated_bonding_curve: associated_bonding_curve_pda,
            associated_user: associated_user_pda,
            system_program: SYSTEM_PROGRAM_ID,
            token_program: TOKEN_PROGRAM_ID,
            creator_vault: creator_vault_pda,
            program: PROGRAM_ID,
            global_volume_accumulator: global_volume_accumulator_pda,
            user_volume_accumulator: user_volume_accumulator_pda,
            fee_config: fee_config_pda,
            fee_program: PUMP_FEES_PROGRAM_ID,
            bonding_curve_v2: bonding_curve_v2_pda_addr,
            buyback_fee_recipient: buyback_vaults[0],
            pump_authority: pump_authority_pda,
        },
    )
    .signer(&user)
    .send_and_confirm()
    .expect("buy should succeed");

    let bonding_curve_before = fetch_bonding_curve(&provider, &bonding_curve_pda)
        .expect("bonding_curve should be readable");
    let user_sol_before = provider.get_balance(&user.address()).unwrap();
    let bonding_curve_lamports_before = provider.get_balance(&bonding_curve_pda).unwrap();
    let fee_recipient_lamports_before = provider.get_balance(&fee_recipient_accounts[0]).unwrap();
    let buyback_lamports_before = provider.get_balance(&buyback_vaults[0]).unwrap();
    let creator_vault_lamports_before = provider.get_balance(&creator_vault_pda).unwrap();

    let sell_amount = buy_amount / 2;
    let expected_gross_sol = expected_gross_sell(
        sell_amount as u128,
        bonding_curve_before.virtual_quote_reserves as u128,
        bonding_curve_before.virtual_token_reserves as u128,
    );
    let expected_protocol_fee = expected_fee_ceil(expected_gross_sol as u128, TEST_PROTOCOL_FEE_BPS);
    let expected_creator_fee = expected_fee_ceil(expected_gross_sol as u128, TEST_CREATOR_FEE_BPS);
    let expected_buyback_share = expected_fee_floor(expected_protocol_fee as u128, TEST_BUYBACK_BASIS_POINTS);
    let expected_fee_recipient_share = expected_protocol_fee - expected_buyback_share;
    let expected_sol_output = expected_gross_sol - expected_protocol_fee - expected_creator_fee;

    build_sell(
        &provider,
        PROGRAM_ID,
        pump_client::SellArgs {
            amount: sell_amount,
            min_sol_output: 1,
            bonding_curve_bump,
            associated_bonding_curve_bump,
            associated_user_bump,
            creator_vault_bump,
            user_volume_accumulator_bump,
            fee_config_bump,
            buyback_index: 0,
            buyback_vault_bump: buyback_bumps[0],
            bonding_curve_v2_bump,
        },
        SellAccounts {
            global: global_pda,
            fee_recipient: fee_recipient_accounts[0],
            mint: mint.address(),
            bonding_curve: bonding_curve_pda,
            user: user.address(),
            associated_bonding_curve: associated_bonding_curve_pda,
            associated_user: associated_user_pda,
            token_program: TOKEN_PROGRAM_ID,
            system_program: SYSTEM_PROGRAM_ID,
            creator_vault: creator_vault_pda,
            program: PROGRAM_ID,
            user_volume_accumulator: user_volume_accumulator_pda,
            fee_config: fee_config_pda,
            fee_program: PUMP_FEES_PROGRAM_ID,
            bonding_curve_v2: bonding_curve_v2_pda_addr,
            buyback_fee_recipient: buyback_vaults[0],
            pump_authority: pump_authority_pda,
        },
    )
    .signer(&user)
    .log()
    .send_and_confirm()
    .expect("sell should succeed");

    let bonding_curve_after = fetch_bonding_curve(&provider, &bonding_curve_pda)
        .expect("bonding_curve should be readable");
    assert_eq!(
        bonding_curve_after.real_token_reserves,
        bonding_curve_before.real_token_reserves + sell_amount,
        "sell should credit real_token_reserves by the sold amount"
    );
    assert_eq!(
        bonding_curve_after.real_quote_reserves,
        bonding_curve_before.real_quote_reserves - expected_gross_sol,
        "sell should debit real_quote_reserves by exactly the formula's gross_sol"
    );
    assert_eq!(
        bonding_curve_after.virtual_quote_reserves,
        bonding_curve_before.virtual_quote_reserves - expected_gross_sol,
        "sell should debit virtual_quote_reserves by exactly the formula's gross_sol"
    );

    let user_sol_after = provider.get_balance(&user.address()).unwrap();
    assert_eq!(
        user_sol_after,
        user_sol_before + expected_sol_output,
        "user should receive exactly gross_sol minus protocol_fee minus creator_fee (network tx fee is paid by the fee payer, not deducted from add_lamports here)"
    );

    let bonding_curve_lamports_after = provider.get_balance(&bonding_curve_pda).unwrap();
    assert_eq!(
        bonding_curve_lamports_after,
        bonding_curve_lamports_before - expected_gross_sol,
        "bonding_curve's native lamport balance should be debited exactly gross_sol"
    );
    let fee_recipient_lamports_after = provider.get_balance(&fee_recipient_accounts[0]).unwrap();
    assert_eq!(
        fee_recipient_lamports_after,
        fee_recipient_lamports_before + expected_fee_recipient_share,
        "fee_recipient should receive exactly protocol_fee minus the buyback carve-out"
    );
    let buyback_lamports_after = provider.get_balance(&buyback_vaults[0]).unwrap();
    assert_eq!(
        buyback_lamports_after, buyback_lamports_before + expected_buyback_share,
        "buyback_fee_recipient should receive exactly the buyback carve-out (0 in this fixture)"
    );
    let creator_vault_lamports_after = provider.get_balance(&creator_vault_pda).unwrap();
    assert_eq!(
        creator_vault_lamports_after,
        creator_vault_lamports_before + expected_creator_fee,
        "creator_vault should receive exactly creator_fee"
    );

    let ata_data = provider
        .get_account_data(&associated_user_pda)
        .expect("user's associated token account should exist");
    let balance = u64::from_le_bytes(ata_data[64..72].try_into().unwrap());
    assert_eq!(
        balance,
        buy_amount - sell_amount,
        "user's token balance should reflect the buy minus the sell"
    );
}

/// SOL-quote path: `bonding_curve.quote_mint` stays `Address::default()`
/// (never literally `WSOL_MINT` — see `buy_v2.rs`'s own `is_sol_quote`
/// check), `quote_mint` account is `WSOL_MINT`, and the actual value
/// transfer uses native lamports. The quote-side ATAs the account list
/// still requires (`associated_quote_bonding_curve`/`associated_quote_user`/
/// `associated_quote_buyback_fee_recipient`) are pre-created with zero
/// balance, since account validation runs regardless of which branch the
/// handler takes.
#[test]
fn buy_v2_purchases_tokens_and_updates_reserves() {
    let provider = setup();
    let (
        global_pda,
        bonding_curve_pda,
        mint,
        bonding_curve_bump,
        creator,
        fee_recipient_accounts,
        buyback_vaults,
        buyback_bumps,
        _authority,
    ) = setup_tradeable_bonding_curve(&provider);

    let wsol_mint = wsol_mint_address();
    provider
        .set_account(&wsol_mint, spl_mint_account_data(None, 9), &TOKEN_PROGRAM_ID, 10_000_000)
        .expect("inject wsol_mint fixture");

    let user = Keypair::new();
    provider.airdrop(&user.address(), 10_000_000_000).unwrap();
    create_ata(&provider, &mint.address(), &user.address()).expect("create user base ATA");
    create_ata(&provider, &wsol_mint, &user.address()).expect("create user quote ATA");

    let (associated_base_bonding_curve_pda, associated_base_bonding_curve_bump) = Address::find_program_address(
        &[bonding_curve_pda.as_ref(), TOKEN_PROGRAM_ID.as_ref(), mint.address().as_ref()],
        &ASSOCIATED_TOKEN_PROGRAM_ID,
    );
    create_ata(&provider, &wsol_mint, &bonding_curve_pda).expect("create bonding_curve quote ATA");
    let (associated_quote_bonding_curve_pda, associated_quote_bonding_curve_bump) = Address::find_program_address(
        &[bonding_curve_pda.as_ref(), TOKEN_PROGRAM_ID.as_ref(), wsol_mint.as_ref()],
        &ASSOCIATED_TOKEN_PROGRAM_ID,
    );
    let (associated_base_user_pda, associated_base_user_bump) = Address::find_program_address(
        &[user.address().as_ref(), TOKEN_PROGRAM_ID.as_ref(), mint.address().as_ref()],
        &ASSOCIATED_TOKEN_PROGRAM_ID,
    );
    let (associated_quote_user_pda, associated_quote_user_bump) = Address::find_program_address(
        &[user.address().as_ref(), TOKEN_PROGRAM_ID.as_ref(), wsol_mint.as_ref()],
        &ASSOCIATED_TOKEN_PROGRAM_ID,
    );
    let (creator_vault_pda, creator_vault_bump) = Address::find_program_address(
        &[CREATOR_VAULT_SEED, creator.as_ref()],
        &PROGRAM_ID,
    );
    let (associated_creator_vault_pda, associated_creator_vault_bump) = Address::find_program_address(
        &[creator_vault_pda.as_ref(), TOKEN_PROGRAM_ID.as_ref(), wsol_mint.as_ref()],
        &ASSOCIATED_TOKEN_PROGRAM_ID,
    );
    let (associated_quote_fee_recipient_pda, associated_quote_fee_recipient_bump) = Address::find_program_address(
        &[fee_recipient_accounts[0].as_ref(), TOKEN_PROGRAM_ID.as_ref(), wsol_mint.as_ref()],
        &ASSOCIATED_TOKEN_PROGRAM_ID,
    );
    create_ata(&provider, &wsol_mint, &buyback_vaults[0]).expect("create buyback quote ATA");
    let (associated_quote_buyback_fee_recipient_pda, associated_quote_buyback_fee_recipient_bump) =
        Address::find_program_address(
            &[buyback_vaults[0].as_ref(), TOKEN_PROGRAM_ID.as_ref(), wsol_mint.as_ref()],
            &ASSOCIATED_TOKEN_PROGRAM_ID,
        );
    let (global_volume_accumulator_pda, _) =
        Address::find_program_address(&[GLOBAL_VOLUME_ACCUMULATOR_SEED], &PROGRAM_ID);
    // Real `buy_v2` never writes this account but does require it to already
    // exist (same as classic `buy` — see `buy_v2.rs`'s own doc comment); the
    // classic `buy` test never explicitly injects it either, so whatever
    // lets that test pass isn't reproducing here — injecting directly rather
    // than continuing to guess why.
    provider
        .set_account(&global_volume_accumulator_pda, global_volume_accumulator_account_data(), &PROGRAM_ID, 10_000_000)
        .expect("inject global_volume_accumulator fixture");
    let (user_volume_accumulator_pda, user_volume_accumulator_bump) = Address::find_program_address(
        &[USER_VOLUME_ACCUMULATOR_SEED, user.address().as_ref()],
        &PROGRAM_ID,
    );
    let (associated_user_volume_accumulator_pda, associated_user_volume_accumulator_bump) =
        Address::find_program_address(
            &[user_volume_accumulator_pda.as_ref(), TOKEN_PROGRAM_ID.as_ref(), wsol_mint.as_ref()],
            &ASSOCIATED_TOKEN_PROGRAM_ID,
        );
    let (fee_config_pda, fee_config_bump) = Address::find_program_address(
        &[FEE_CONFIG_SEED, PROGRAM_ID.as_ref()],
        &PUMP_FEES_PROGRAM_ID,
    );
    let (pump_authority_pda, _) = Address::find_program_address(&[PUMP_AUTHORITY_SEED], &PROGRAM_ID);

    let bonding_curve_before = fetch_bonding_curve(&provider, &bonding_curve_pda)
        .expect("bonding_curve should be readable");

    let amount = 15_000_000_000_000u64;
    let expected_net_quote = expected_gross_buy(
        amount as u128,
        bonding_curve_before.virtual_quote_reserves as u128,
        bonding_curve_before.virtual_token_reserves as u128,
    );
    let expected_protocol_fee = expected_fee_ceil(expected_net_quote as u128, TEST_PROTOCOL_FEE_BPS);
    let expected_creator_fee = expected_fee_ceil(expected_net_quote as u128, TEST_CREATOR_FEE_BPS);
    let expected_buyback_share = expected_fee_floor(expected_protocol_fee as u128, TEST_BUYBACK_BASIS_POINTS);
    let expected_fee_recipient_share = expected_protocol_fee - expected_buyback_share;
    let expected_total_cost = expected_net_quote + expected_protocol_fee + expected_creator_fee;

    let user_sol_before = provider.get_balance(&user.address()).unwrap();
    let bonding_curve_lamports_before = provider.get_balance(&bonding_curve_pda).unwrap();
    let fee_recipient_lamports_before = provider.get_balance(&fee_recipient_accounts[0]).unwrap();
    let buyback_lamports_before = provider.get_balance(&buyback_vaults[0]).unwrap();

    build_buy_v2(
        &provider,
        PROGRAM_ID,
        BuyV2Args {
            amount,
            max_sol_cost: 1_000_000_000,
            bonding_curve_bump,
            associated_base_bonding_curve_bump,
            associated_quote_bonding_curve_bump,
            associated_base_user_bump,
            associated_quote_user_bump,
            creator_vault_bump,
            associated_creator_vault_bump,
            associated_quote_fee_recipient_bump,
            associated_quote_buyback_fee_recipient_bump,
            user_volume_accumulator_bump,
            associated_user_volume_accumulator_bump,
            fee_config_bump,
            buyback_index: 0,
            buyback_vault_bump: buyback_bumps[0],
        },
        BuyV2Accounts {
            global: global_pda,
            base_mint: mint.address(),
            quote_mint: wsol_mint,
            base_token_program: TOKEN_PROGRAM_ID,
            quote_token_program: TOKEN_PROGRAM_ID,
            associated_token_program: ASSOCIATED_TOKEN_PROGRAM_ID,
            fee_recipient: fee_recipient_accounts[0],
            associated_quote_fee_recipient: associated_quote_fee_recipient_pda,
            buyback_fee_recipient: buyback_vaults[0],
            associated_quote_buyback_fee_recipient: associated_quote_buyback_fee_recipient_pda,
            bonding_curve: bonding_curve_pda,
            associated_base_bonding_curve: associated_base_bonding_curve_pda,
            associated_quote_bonding_curve: associated_quote_bonding_curve_pda,
            user: user.address(),
            associated_base_user: associated_base_user_pda,
            associated_quote_user: associated_quote_user_pda,
            creator_vault: creator_vault_pda,
            associated_creator_vault: associated_creator_vault_pda,
            sharing_config: Address::default(),
            global_volume_accumulator: global_volume_accumulator_pda,
            user_volume_accumulator: user_volume_accumulator_pda,
            associated_user_volume_accumulator: associated_user_volume_accumulator_pda,
            program: PROGRAM_ID,
            fee_config: fee_config_pda,
            fee_program: PUMP_FEES_PROGRAM_ID,
            system_program: SYSTEM_PROGRAM_ID,
            pump_authority: pump_authority_pda,
        },
    )
    .signer(&user)
    .log()
    .send_and_confirm()
    .expect("buy_v2 should succeed");

    let bonding_curve_after = fetch_bonding_curve(&provider, &bonding_curve_pda)
        .expect("bonding_curve should be readable");
    assert_eq!(
        bonding_curve_after.real_token_reserves,
        bonding_curve_before.real_token_reserves - amount,
        "buy_v2 should debit real_token_reserves by the purchased amount"
    );
    assert_eq!(
        bonding_curve_after.real_quote_reserves,
        bonding_curve_before.real_quote_reserves + expected_net_quote,
        "buy_v2 should credit real_quote_reserves by exactly the formula's net_quote"
    );
    assert_eq!(
        bonding_curve_after.virtual_quote_reserves,
        bonding_curve_before.virtual_quote_reserves + expected_net_quote,
        "buy_v2 should credit virtual_quote_reserves by exactly the formula's net_quote"
    );

    assert_eq!(
        token_balance(&provider, &associated_base_user_pda),
        amount,
        "user should have received the purchased tokens"
    );

    // Real `buy_v2`, confirmed via `reference/fee-tier-probe/src/bin/probe51.rs`
    // against real deployed bytecode, never touches any quote-side WSOL ATA
    // for a SOL-paired coin — the real fund flow is native lamports, same as
    // classic `buy`. See `buy_v2.rs`'s own module comment.
    assert_eq!(token_balance(&provider, &associated_quote_bonding_curve_pda), 0, "associated_quote_bonding_curve should never be touched for a SOL-paired coin");
    assert_eq!(token_balance(&provider, &associated_quote_fee_recipient_pda), 0, "associated_quote_fee_recipient should never be touched for a SOL-paired coin");
    assert_eq!(token_balance(&provider, &associated_quote_buyback_fee_recipient_pda), 0, "associated_quote_buyback_fee_recipient should never be touched for a SOL-paired coin");
    assert_eq!(token_balance(&provider, &associated_creator_vault_pda), 0, "associated_creator_vault should never be touched for a SOL-paired coin");
    assert_eq!(token_balance(&provider, &associated_quote_user_pda), 0, "associated_quote_user should never be touched for a SOL-paired coin");

    let bonding_curve_lamports_after = provider.get_balance(&bonding_curve_pda).unwrap();
    assert_eq!(
        bonding_curve_lamports_after,
        bonding_curve_lamports_before + expected_net_quote,
        "bonding_curve's native lamport balance should be credited exactly net_quote"
    );
    let fee_recipient_lamports_after = provider.get_balance(&fee_recipient_accounts[0]).unwrap();
    assert_eq!(
        fee_recipient_lamports_after,
        fee_recipient_lamports_before + expected_fee_recipient_share,
        "fee_recipient should receive exactly protocol_fee minus the buyback carve-out"
    );
    let buyback_lamports_after = provider.get_balance(&buyback_vaults[0]).unwrap();
    assert_eq!(
        buyback_lamports_after, buyback_lamports_before + expected_buyback_share,
        "buyback_fee_recipient should receive exactly the buyback carve-out (0 in this fixture)"
    );
    // `creator_vault` starts completely empty (this is the very first trade
    // on this curve), so the confirmed conditional rent-exempt top-up fires:
    // exactly `CREATOR_VAULT_RENT_EXEMPT_MINIMUM` lamports, on top of
    // `creator_fee` itself (see `buy_v2.rs`'s module comment for how this
    // exact formula was confirmed via probe51.rs).
    let creator_vault_lamports_after = provider.get_balance(&creator_vault_pda).unwrap();
    assert_eq!(
        creator_vault_lamports_after,
        CREATOR_VAULT_RENT_EXEMPT_MINIMUM + expected_creator_fee,
        "creator_vault should receive the rent-exempt top-up plus exactly creator_fee"
    );

    // `associated_quote_fee_recipient`, `associated_creator_vault`,
    // `user_volume_accumulator`, and `associated_user_volume_accumulator`
    // are all created fresh here via `init_if_needed`, payer = user — real,
    // legitimate one-time rent, but no part of the swap formula itself
    // (`global_volume_accumulator` is pre-injected by the test fixture, not
    // `init_if_needed`, so it costs `user` nothing here).
    let rent_paid_for_new_accounts = provider.get_balance(&associated_quote_fee_recipient_pda).unwrap()
        + provider.get_balance(&associated_creator_vault_pda).unwrap()
        + provider.get_balance(&user_volume_accumulator_pda).unwrap()
        + provider.get_balance(&associated_user_volume_accumulator_pda).unwrap();
    let user_sol_after = provider.get_balance(&user.address()).unwrap();
    assert_eq!(
        user_sol_before - user_sol_after,
        expected_total_cost + CREATOR_VAULT_RENT_EXEMPT_MINIMUM + rent_paid_for_new_accounts,
        "user should pay exactly net_quote + protocol_fee + creator_fee in native SOL, plus creator_vault's rent-exempt top-up, plus the one-time rent for newly-created accounts (the tx fee payer is a separate wallet, not `user`)"
    );
}

#[test]
fn sell_v2_returns_tokens_and_updates_reserves() {
    let provider = setup();
    let (
        global_pda,
        bonding_curve_pda,
        mint,
        bonding_curve_bump,
        creator,
        fee_recipient_accounts,
        buyback_vaults,
        buyback_bumps,
        _authority,
    ) = setup_tradeable_bonding_curve(&provider);

    let wsol_mint = wsol_mint_address();
    provider
        .set_account(&wsol_mint, spl_mint_account_data(None, 9), &TOKEN_PROGRAM_ID, 10_000_000)
        .expect("inject wsol_mint fixture");

    let user = Keypair::new();
    provider.airdrop(&user.address(), 10_000_000_000).unwrap();
    create_ata(&provider, &mint.address(), &user.address()).expect("create user base ATA");
    create_ata(&provider, &wsol_mint, &user.address()).expect("create user quote ATA");

    let (associated_base_bonding_curve_pda, associated_base_bonding_curve_bump) = Address::find_program_address(
        &[bonding_curve_pda.as_ref(), TOKEN_PROGRAM_ID.as_ref(), mint.address().as_ref()],
        &ASSOCIATED_TOKEN_PROGRAM_ID,
    );
    create_ata(&provider, &wsol_mint, &bonding_curve_pda).expect("create bonding_curve quote ATA");
    let (associated_quote_bonding_curve_pda, associated_quote_bonding_curve_bump) = Address::find_program_address(
        &[bonding_curve_pda.as_ref(), TOKEN_PROGRAM_ID.as_ref(), wsol_mint.as_ref()],
        &ASSOCIATED_TOKEN_PROGRAM_ID,
    );
    let (associated_base_user_pda, associated_base_user_bump) = Address::find_program_address(
        &[user.address().as_ref(), TOKEN_PROGRAM_ID.as_ref(), mint.address().as_ref()],
        &ASSOCIATED_TOKEN_PROGRAM_ID,
    );
    let (associated_quote_user_pda, associated_quote_user_bump) = Address::find_program_address(
        &[user.address().as_ref(), TOKEN_PROGRAM_ID.as_ref(), wsol_mint.as_ref()],
        &ASSOCIATED_TOKEN_PROGRAM_ID,
    );
    let (creator_vault_pda, creator_vault_bump) = Address::find_program_address(
        &[CREATOR_VAULT_SEED, creator.as_ref()],
        &PROGRAM_ID,
    );
    let (associated_creator_vault_pda, associated_creator_vault_bump) = Address::find_program_address(
        &[creator_vault_pda.as_ref(), TOKEN_PROGRAM_ID.as_ref(), wsol_mint.as_ref()],
        &ASSOCIATED_TOKEN_PROGRAM_ID,
    );
    let (associated_quote_fee_recipient_pda, associated_quote_fee_recipient_bump) = Address::find_program_address(
        &[fee_recipient_accounts[0].as_ref(), TOKEN_PROGRAM_ID.as_ref(), wsol_mint.as_ref()],
        &ASSOCIATED_TOKEN_PROGRAM_ID,
    );
    create_ata(&provider, &wsol_mint, &buyback_vaults[0]).expect("create buyback quote ATA");
    let (associated_quote_buyback_fee_recipient_pda, associated_quote_buyback_fee_recipient_bump) =
        Address::find_program_address(
            &[buyback_vaults[0].as_ref(), TOKEN_PROGRAM_ID.as_ref(), wsol_mint.as_ref()],
            &ASSOCIATED_TOKEN_PROGRAM_ID,
        );
    let (global_volume_accumulator_pda, _) =
        Address::find_program_address(&[GLOBAL_VOLUME_ACCUMULATOR_SEED], &PROGRAM_ID);
    provider
        .set_account(&global_volume_accumulator_pda, global_volume_accumulator_account_data(), &PROGRAM_ID, 10_000_000)
        .expect("inject global_volume_accumulator fixture");
    let (user_volume_accumulator_pda, user_volume_accumulator_bump) = Address::find_program_address(
        &[USER_VOLUME_ACCUMULATOR_SEED, user.address().as_ref()],
        &PROGRAM_ID,
    );
    let (associated_user_volume_accumulator_pda, associated_user_volume_accumulator_bump) =
        Address::find_program_address(
            &[user_volume_accumulator_pda.as_ref(), TOKEN_PROGRAM_ID.as_ref(), wsol_mint.as_ref()],
            &ASSOCIATED_TOKEN_PROGRAM_ID,
        );
    let (fee_config_pda, fee_config_bump) = Address::find_program_address(
        &[FEE_CONFIG_SEED, PROGRAM_ID.as_ref()],
        &PUMP_FEES_PROGRAM_ID,
    );
    let (pump_authority_pda, _) = Address::find_program_address(&[PUMP_AUTHORITY_SEED], &PROGRAM_ID);

    let buy_amount = 15_000_000_000_000u64;
    build_buy_v2(
        &provider,
        PROGRAM_ID,
        BuyV2Args {
            amount: buy_amount,
            max_sol_cost: 1_000_000_000,
            bonding_curve_bump,
            associated_base_bonding_curve_bump,
            associated_quote_bonding_curve_bump,
            associated_base_user_bump,
            associated_quote_user_bump,
            creator_vault_bump,
            associated_creator_vault_bump,
            associated_quote_fee_recipient_bump,
            associated_quote_buyback_fee_recipient_bump,
            user_volume_accumulator_bump,
            associated_user_volume_accumulator_bump,
            fee_config_bump,
            buyback_index: 0,
            buyback_vault_bump: buyback_bumps[0],
        },
        BuyV2Accounts {
            global: global_pda,
            base_mint: mint.address(),
            quote_mint: wsol_mint,
            base_token_program: TOKEN_PROGRAM_ID,
            quote_token_program: TOKEN_PROGRAM_ID,
            associated_token_program: ASSOCIATED_TOKEN_PROGRAM_ID,
            fee_recipient: fee_recipient_accounts[0],
            associated_quote_fee_recipient: associated_quote_fee_recipient_pda,
            buyback_fee_recipient: buyback_vaults[0],
            associated_quote_buyback_fee_recipient: associated_quote_buyback_fee_recipient_pda,
            bonding_curve: bonding_curve_pda,
            associated_base_bonding_curve: associated_base_bonding_curve_pda,
            associated_quote_bonding_curve: associated_quote_bonding_curve_pda,
            user: user.address(),
            associated_base_user: associated_base_user_pda,
            associated_quote_user: associated_quote_user_pda,
            creator_vault: creator_vault_pda,
            associated_creator_vault: associated_creator_vault_pda,
            sharing_config: Address::default(),
            global_volume_accumulator: global_volume_accumulator_pda,
            user_volume_accumulator: user_volume_accumulator_pda,
            associated_user_volume_accumulator: associated_user_volume_accumulator_pda,
            program: PROGRAM_ID,
            fee_config: fee_config_pda,
            fee_program: PUMP_FEES_PROGRAM_ID,
            system_program: SYSTEM_PROGRAM_ID,
            pump_authority: pump_authority_pda,
        },
    )
    .signer(&user)
    .send_and_confirm()
    .expect("buy_v2 setup trade should succeed");

    let bonding_curve_before = fetch_bonding_curve(&provider, &bonding_curve_pda)
        .expect("bonding_curve should be readable");

    let sell_amount = buy_amount / 2;
    let expected_gross_sol = expected_gross_sell(
        sell_amount as u128,
        bonding_curve_before.virtual_quote_reserves as u128,
        bonding_curve_before.virtual_token_reserves as u128,
    );
    let expected_protocol_fee = expected_fee_ceil(expected_gross_sol as u128, TEST_PROTOCOL_FEE_BPS);
    let expected_creator_fee = expected_fee_ceil(expected_gross_sol as u128, TEST_CREATOR_FEE_BPS);
    let expected_buyback_share = expected_fee_floor(expected_protocol_fee as u128, TEST_BUYBACK_BASIS_POINTS);
    let expected_fee_recipient_share = expected_protocol_fee - expected_buyback_share;
    let expected_sol_output = expected_gross_sol - expected_protocol_fee - expected_creator_fee;

    let bonding_curve_lamports_before = provider.get_balance(&bonding_curve_pda).unwrap();
    let fee_recipient_lamports_before = provider.get_balance(&fee_recipient_accounts[0]).unwrap();
    let buyback_lamports_before = provider.get_balance(&buyback_vaults[0]).unwrap();
    let creator_vault_lamports_before = provider.get_balance(&creator_vault_pda).unwrap();
    let user_sol_before = provider.get_balance(&user.address()).unwrap();

    build_sell_v2(
        &provider,
        PROGRAM_ID,
        SellV2Args {
            amount: sell_amount,
            min_sol_output: 0,
            bonding_curve_bump,
            associated_base_bonding_curve_bump,
            associated_quote_bonding_curve_bump,
            associated_base_user_bump,
            associated_quote_user_bump,
            creator_vault_bump,
            associated_creator_vault_bump,
            associated_quote_fee_recipient_bump,
            associated_quote_buyback_fee_recipient_bump,
            user_volume_accumulator_bump,
            associated_user_volume_accumulator_bump,
            fee_config_bump,
            buyback_index: 0,
            buyback_vault_bump: buyback_bumps[0],
        },
        SellV2Accounts {
            global: global_pda,
            base_mint: mint.address(),
            quote_mint: wsol_mint,
            base_token_program: TOKEN_PROGRAM_ID,
            quote_token_program: TOKEN_PROGRAM_ID,
            associated_token_program: ASSOCIATED_TOKEN_PROGRAM_ID,
            fee_recipient: fee_recipient_accounts[0],
            associated_quote_fee_recipient: associated_quote_fee_recipient_pda,
            buyback_fee_recipient: buyback_vaults[0],
            associated_quote_buyback_fee_recipient: associated_quote_buyback_fee_recipient_pda,
            bonding_curve: bonding_curve_pda,
            associated_base_bonding_curve: associated_base_bonding_curve_pda,
            associated_quote_bonding_curve: associated_quote_bonding_curve_pda,
            user: user.address(),
            associated_base_user: associated_base_user_pda,
            associated_quote_user: associated_quote_user_pda,
            creator_vault: creator_vault_pda,
            associated_creator_vault: associated_creator_vault_pda,
            sharing_config: Address::default(),
            user_volume_accumulator: user_volume_accumulator_pda,
            associated_user_volume_accumulator: associated_user_volume_accumulator_pda,
            program: PROGRAM_ID,
            fee_config: fee_config_pda,
            fee_program: PUMP_FEES_PROGRAM_ID,
            system_program: SYSTEM_PROGRAM_ID,
            pump_authority: pump_authority_pda,
        },
    )
    .signer(&user)
    .log()
    .send_and_confirm()
    .expect("sell_v2 should succeed");

    let bonding_curve_after = fetch_bonding_curve(&provider, &bonding_curve_pda)
        .expect("bonding_curve should be readable");
    assert_eq!(
        bonding_curve_after.real_token_reserves,
        bonding_curve_before.real_token_reserves + sell_amount,
        "sell_v2 should credit real_token_reserves by the sold amount"
    );
    assert_eq!(
        bonding_curve_after.real_quote_reserves,
        bonding_curve_before.real_quote_reserves - expected_gross_sol,
        "sell_v2 should debit real_quote_reserves by exactly the formula's gross_sol"
    );
    assert_eq!(
        bonding_curve_after.virtual_quote_reserves,
        bonding_curve_before.virtual_quote_reserves - expected_gross_sol,
        "sell_v2 should debit virtual_quote_reserves by exactly the formula's gross_sol"
    );

    assert_eq!(
        token_balance(&provider, &associated_base_user_pda),
        buy_amount - sell_amount,
        "user's base balance should reflect the buy then the partial sell"
    );

    // Real `sell_v2`, confirmed via `reference/fee-tier-probe/src/bin/probe51.rs`
    // against real deployed bytecode, never touches any quote-side WSOL ATA
    // for a SOL-paired coin — the real fund flow is native lamports, same as
    // classic `sell`. See `sell_v2.rs`'s own module comment.
    assert_eq!(token_balance(&provider, &associated_quote_bonding_curve_pda), 0, "associated_quote_bonding_curve should never be touched for a SOL-paired coin");
    assert_eq!(token_balance(&provider, &associated_quote_fee_recipient_pda), 0, "associated_quote_fee_recipient should never be touched for a SOL-paired coin");
    assert_eq!(token_balance(&provider, &associated_quote_buyback_fee_recipient_pda), 0, "associated_quote_buyback_fee_recipient should never be touched for a SOL-paired coin");
    assert_eq!(token_balance(&provider, &associated_creator_vault_pda), 0, "associated_creator_vault should never be touched for a SOL-paired coin");
    assert_eq!(token_balance(&provider, &associated_quote_user_pda), 0, "associated_quote_user should never be touched for a SOL-paired coin");

    let bonding_curve_lamports_after = provider.get_balance(&bonding_curve_pda).unwrap();
    assert_eq!(
        bonding_curve_lamports_after,
        bonding_curve_lamports_before - expected_gross_sol,
        "bonding_curve's native lamport balance should be debited exactly gross_sol"
    );
    let fee_recipient_lamports_after = provider.get_balance(&fee_recipient_accounts[0]).unwrap();
    assert_eq!(
        fee_recipient_lamports_after,
        fee_recipient_lamports_before + expected_fee_recipient_share,
        "fee_recipient should receive exactly protocol_fee minus the buyback carve-out"
    );
    let buyback_lamports_after = provider.get_balance(&buyback_vaults[0]).unwrap();
    assert_eq!(
        buyback_lamports_after, buyback_lamports_before + expected_buyback_share,
        "buyback_fee_recipient should receive exactly the buyback carve-out (0 in this fixture)"
    );
    // `creator_vault` is already funded above the rent-exempt minimum from
    // the setup `buy_v2` call, so the conditional top-up should NOT fire
    // again here — confirmed real behavior via probe51.rs (a second real
    // trade on an already-funded creator_vault added no top-up at all).
    let expected_creator_vault_topup = if creator_vault_lamports_before < CREATOR_VAULT_RENT_EXEMPT_MINIMUM {
        CREATOR_VAULT_RENT_EXEMPT_MINIMUM - creator_vault_lamports_before
    } else {
        0
    };
    let creator_vault_lamports_after = provider.get_balance(&creator_vault_pda).unwrap();
    assert_eq!(
        creator_vault_lamports_after,
        creator_vault_lamports_before + expected_creator_vault_topup + expected_creator_fee,
        "creator_vault should receive no top-up (already rent-exempt) plus exactly creator_fee"
    );
    let user_sol_after = provider.get_balance(&user.address()).unwrap();
    assert_eq!(
        user_sol_after,
        user_sol_before + expected_sol_output,
        "user should receive exactly gross_sol minus protocol_fee minus creator_fee in native SOL"
    );
}

/// Real end-to-end `migrate`: graduates a bonding curve by buying its entire
/// `real_token_reserves` in one trade (triggering `complete = true` via
/// `buy`'s own existing logic, same as a real pump.fun graduation), enables
/// migration via `set_params`, then calls `migrate` and checks the confirmed
/// mechanism (`reference/fee-tier-probe/src/bin/probe31-36.rs`) end to end:
/// `associated_bonding_curve`'s full remaining token balance (`token_total_supply
/// - full_amount` — not the `real_token_reserves` counter, which legitimately
/// reaches zero at graduation, confirmed via `probe35`/`probe36`) lands in
/// `pool_base_token_account`, `real_quote_reserves - pool_migration_fee` lands
/// in `pool_quote_token_account` (as WSOL), the bonding curve's reserves are
/// zeroed, and the migrated pool's LP-token account no longer exists (burned +
/// closed).
#[test]
fn migrate_moves_bonding_curve_reserves_into_a_new_pump_amm_pool() {
    let provider = setup();
    load_pump_amm_program(&provider);
    let (
        global_pda,
        bonding_curve_pda,
        mint,
        bonding_curve_bump,
        creator,
        fee_recipient_accounts,
        buyback_vaults,
        buyback_bumps,
        authority,
    ) = setup_tradeable_bonding_curve(&provider);

    let user = Keypair::new();
    provider.airdrop(&user.address(), 200_000_000_000).unwrap();
    create_ata(&provider, &mint.address(), &user.address()).expect("create user ATA");

    let (associated_bonding_curve_pda, associated_bonding_curve_bump) = Address::find_program_address(
        &[bonding_curve_pda.as_ref(), TOKEN_PROGRAM_ID.as_ref(), mint.address().as_ref()],
        &ASSOCIATED_TOKEN_PROGRAM_ID,
    );
    let (associated_user_pda, associated_user_bump) = Address::find_program_address(
        &[user.address().as_ref(), TOKEN_PROGRAM_ID.as_ref(), mint.address().as_ref()],
        &ASSOCIATED_TOKEN_PROGRAM_ID,
    );
    let (creator_vault_pda, creator_vault_bump) = Address::find_program_address(
        &[CREATOR_VAULT_SEED, creator.as_ref()],
        &PROGRAM_ID,
    );
    let (global_volume_accumulator_pda, _) =
        Address::find_program_address(&[GLOBAL_VOLUME_ACCUMULATOR_SEED], &PROGRAM_ID);
    let (user_volume_accumulator_pda, user_volume_accumulator_bump) = Address::find_program_address(
        &[USER_VOLUME_ACCUMULATOR_SEED, user.address().as_ref()],
        &PROGRAM_ID,
    );
    let (fee_config_pda, fee_config_bump) = Address::find_program_address(
        &[FEE_CONFIG_SEED, PROGRAM_ID.as_ref()],
        &PUMP_FEES_PROGRAM_ID,
    );
    let (pump_authority_pda, _) = Address::find_program_address(&[PUMP_AUTHORITY_SEED], &PROGRAM_ID);
    let (bonding_curve_v2_pda_addr, bonding_curve_v2_bump) = bonding_curve_v2_pda(&mint.address());

    // Buy the curve's ENTIRE `real_token_reserves` in one trade — real
    // pump-fun graduation economics, matches `reference/fee-tier-probe`'s own
    // `REAL_SOL_RESERVES = 85_000_000_000` figure closely.
    let full_amount = 793_100_000_000_000u64;
    build_buy(
        &provider,
        PROGRAM_ID,
        pump_client::BuyArgs {
            amount: full_amount,
            max_sol_cost: 150_000_000_000,
            track_volume: Bool::from(false),
            bonding_curve_bump,
            associated_bonding_curve_bump,
            associated_user_bump,
            creator_vault_bump,
            user_volume_accumulator_bump,
            fee_config_bump,
            buyback_index: 0,
            buyback_vault_bump: buyback_bumps[0],
            bonding_curve_v2_bump,
        },
        BuyAccounts {
            global: global_pda,
            fee_recipient: fee_recipient_accounts[0],
            mint: mint.address(),
            bonding_curve: bonding_curve_pda,
            user: user.address(),
            associated_bonding_curve: associated_bonding_curve_pda,
            associated_user: associated_user_pda,
            system_program: SYSTEM_PROGRAM_ID,
            token_program: TOKEN_PROGRAM_ID,
            creator_vault: creator_vault_pda,
            program: PROGRAM_ID,
            global_volume_accumulator: global_volume_accumulator_pda,
            user_volume_accumulator: user_volume_accumulator_pda,
            fee_config: fee_config_pda,
            fee_program: PUMP_FEES_PROGRAM_ID,
            bonding_curve_v2: bonding_curve_v2_pda_addr,
            buyback_fee_recipient: buyback_vaults[0],
            pump_authority: pump_authority_pda,
        },
    )
    .signer(&user)
    .log()
    .send_and_confirm()
    .expect("graduating buy should succeed");

    let graduated = fetch_bonding_curve(&provider, &bonding_curve_pda)
        .expect("bonding_curve should be readable");
    assert!(bool::from(graduated.complete), "bonding curve should be complete after buying out its full reserves");
    assert_eq!(graduated.real_token_reserves, 0);
    let real_quote_reserves = graduated.real_quote_reserves;

    // Enable migration. `15_000_001` is the real `pump.so`'s own real-world
    // value (confirmed via `probe33.rs`), but `pool_migration_fee` is a
    // runtime-configurable admin parameter, not a hardcoded constant, and
    // our reimplementation's account sizes (e.g. `Pool`'s own component
    // layout) aren't guaranteed byte-identical to the real bytecode's — so
    // this test uses a deliberately generous budget rather than the exact
    // real-world figure, to isolate migration-logic correctness from
    // rent-sizing precision.
    let withdraw_authority = Keypair::new().address();
    let pool_migration_fee: u64 = 30_000_000;
    build_set_params(
        &provider,
        PROGRAM_ID,
        pump_client::SetParamsArgs {
            initial_virtual_token_reserves: 1_073_000_000_000_000,
            initial_virtual_sol_reserves: 30_000_000_000,
            initial_real_token_reserves: 793_100_000_000_000,
            token_total_supply: 1_000_000_000_000_000,
            fee_basis_points: 0,
            withdraw_authority,
            enable_migrate: Bool::from(true),
            pool_migration_fee,
            creator_fee_basis_points: 0,
            set_creator_authority: Address::default(),
            admin_set_creator_authority: Address::default(),
        },
        SetParamsAccounts { global: global_pda, authority: authority.address() },
    )
    .signer(&authority)
    .remaining_accounts(
        fee_recipient_accounts
            .iter()
            .map(|address| AccountMeta { address: *address, is_signer: false, is_writable: false })
            .collect(),
    )
    .send_and_confirm()
    .expect("set_params should succeed enabling migrate");

    // Fixtures `migrate`'s nested `create_pool` CPI needs: a real WSOL mint
    // and `pump_amm`'s own (scoped) `GlobalConfig` with `disable_flags = 0`.
    let wsol_mint = wsol_mint_address();
    provider
        .set_account(&wsol_mint, spl_mint_account_data(None, 9), &TOKEN_PROGRAM_ID, 10_000_000)
        .expect("inject wsol_mint fixture");
    let (amm_global_config_pda, amm_global_config_bump) =
        Address::find_program_address(&[GLOBAL_CONFIG_SEED], &PUMP_AMM_PROGRAM_ID);
    provider
        .set_account(&amm_global_config_pda, global_config_account_data(0), &PUMP_AMM_PROGRAM_ID, 10_000_000)
        .expect("inject amm_global_config fixture");

    let (pool_authority_pda, pool_authority_bump) =
        Address::find_program_address(&[POOL_AUTHORITY_SEED, mint.address().as_ref()], &PROGRAM_ID);
    let (pool_pda, pool_bump) = Address::find_program_address(
        &[POOL_SEED, &0u16.to_le_bytes(), pool_authority_pda.as_ref(), mint.address().as_ref(), wsol_mint.as_ref()],
        &PUMP_AMM_PROGRAM_ID,
    );
    let (pool_authority_mint_account_pda, pool_authority_mint_account_bump) = Address::find_program_address(
        &[pool_authority_pda.as_ref(), TOKEN_PROGRAM_ID.as_ref(), mint.address().as_ref()],
        &ASSOCIATED_TOKEN_PROGRAM_ID,
    );
    let (pool_authority_wsol_account_pda, pool_authority_wsol_account_bump) = Address::find_program_address(
        &[pool_authority_pda.as_ref(), TOKEN_PROGRAM_ID.as_ref(), wsol_mint.as_ref()],
        &ASSOCIATED_TOKEN_PROGRAM_ID,
    );
    let (lp_mint_pda, lp_mint_bump) =
        Address::find_program_address(&[POOL_LP_MINT_SEED, pool_pda.as_ref()], &PUMP_AMM_PROGRAM_ID);
    let (user_pool_token_account_pda, user_pool_token_account_bump) = Address::find_program_address(
        &[pool_authority_pda.as_ref(), TOKEN_2022_PROGRAM_ID.as_ref(), lp_mint_pda.as_ref()],
        &ASSOCIATED_TOKEN_PROGRAM_ID,
    );
    let (pool_base_token_account_pda, pool_base_token_account_bump) = Address::find_program_address(
        &[pool_pda.as_ref(), TOKEN_PROGRAM_ID.as_ref(), mint.address().as_ref()],
        &ASSOCIATED_TOKEN_PROGRAM_ID,
    );
    let (pool_quote_token_account_pda, pool_quote_token_account_bump) = Address::find_program_address(
        &[pool_pda.as_ref(), TOKEN_PROGRAM_ID.as_ref(), wsol_mint.as_ref()],
        &ASSOCIATED_TOKEN_PROGRAM_ID,
    );

    let withdraw_authority_lamports_before = provider.get_balance(&withdraw_authority).unwrap_or(0);

    build_migrate(
        &provider,
        PROGRAM_ID,
        pump_client::MigrateArgs {
            bonding_curve_bump,
            associated_bonding_curve_bump,
            pool_authority_bump,
            pool_authority_mint_account_bump,
            pool_authority_wsol_account_bump,
            amm_global_config_bump,
            pool_bump,
            lp_mint_bump,
            user_pool_token_account_bump,
            pool_base_token_account_bump,
            pool_quote_token_account_bump,
        },
        MigrateAccounts {
            global: global_pda,
            withdraw_authority,
            mint: mint.address(),
            bonding_curve: bonding_curve_pda,
            wsol_mint,
            user: user.address(),
            system_program: SYSTEM_PROGRAM_ID,
            token_program: TOKEN_PROGRAM_ID,
            token_2022_program: TOKEN_2022_PROGRAM_ID,
            associated_token_program: ASSOCIATED_TOKEN_PROGRAM_ID,
            pump_amm: PUMP_AMM_PROGRAM_ID,
            rent: RENT_SYSVAR_ID,
            associated_bonding_curve: associated_bonding_curve_pda,
            pool_authority: pool_authority_pda,
            pool: pool_pda,
            pool_authority_mint_account: pool_authority_mint_account_pda,
            pool_authority_wsol_account: pool_authority_wsol_account_pda,
            amm_global_config: amm_global_config_pda,
            lp_mint: lp_mint_pda,
            user_pool_token_account: user_pool_token_account_pda,
            pool_base_token_account: pool_base_token_account_pda,
            pool_quote_token_account: pool_quote_token_account_pda,
        },
    )
    .signer(&user)
    .log()
    .send_and_confirm()
    .expect("migrate should succeed");

    let pool_base_balance = u64::from_le_bytes(
        provider.get_account_data(&pool_base_token_account_pda).expect("pool_base_token_account should exist")[64..72]
            .try_into()
            .unwrap(),
    );
    let token_total_supply = 1_000_000_000_000_000u64;
    assert_eq!(
        pool_base_balance,
        token_total_supply - full_amount,
        "associated_bonding_curve's full remaining token balance should land in pool_base_token_account"
    );

    let pool_quote_balance = u64::from_le_bytes(
        provider.get_account_data(&pool_quote_token_account_pda).expect("pool_quote_token_account should exist")[64..72]
            .try_into()
            .unwrap(),
    );
    assert_eq!(
        pool_quote_balance,
        real_quote_reserves - pool_migration_fee,
        "pool_quote_token_account should hold real_quote_reserves minus pool_migration_fee"
    );

    let migrated = fetch_bonding_curve(&provider, &bonding_curve_pda)
        .expect("bonding_curve should still be readable after migrate");
    assert_eq!(migrated.real_token_reserves, 0);
    assert_eq!(migrated.real_quote_reserves, 0);
    assert_eq!(migrated.virtual_token_reserves, 0);
    assert_eq!(migrated.virtual_quote_reserves, 0);

    assert!(
        provider.get_account_data(&user_pool_token_account_pda).is_err(),
        "pool_authority's LP-token account should have been burned and closed"
    );

    let withdraw_authority_lamports_after = provider.get_balance(&withdraw_authority).unwrap();
    assert!(
        withdraw_authority_lamports_after > withdraw_authority_lamports_before,
        "withdraw_authority should receive pool_authority's leftover pool_migration_fee"
    );

    // Real, confirmed via `reference/fee-tier-probe/src/bin/probe56.rs`
    // through `probe63.rs`: calling `migrate` again on an already-migrated
    // curve succeeds as a real no-op (logs "Bonding curve already
    // migrated", runs zero CPIs) rather than erroring.
    provider.expire_blockhash().unwrap();
    build_migrate(
        &provider,
        PROGRAM_ID,
        pump_client::MigrateArgs {
            bonding_curve_bump,
            associated_bonding_curve_bump,
            pool_authority_bump,
            pool_authority_mint_account_bump,
            pool_authority_wsol_account_bump,
            amm_global_config_bump,
            pool_bump,
            lp_mint_bump,
            user_pool_token_account_bump,
            pool_base_token_account_bump,
            pool_quote_token_account_bump,
        },
        MigrateAccounts {
            global: global_pda,
            withdraw_authority,
            mint: mint.address(),
            bonding_curve: bonding_curve_pda,
            wsol_mint,
            user: user.address(),
            system_program: SYSTEM_PROGRAM_ID,
            token_program: TOKEN_PROGRAM_ID,
            token_2022_program: TOKEN_2022_PROGRAM_ID,
            associated_token_program: ASSOCIATED_TOKEN_PROGRAM_ID,
            pump_amm: PUMP_AMM_PROGRAM_ID,
            rent: RENT_SYSVAR_ID,
            associated_bonding_curve: associated_bonding_curve_pda,
            pool_authority: pool_authority_pda,
            pool: pool_pda,
            pool_authority_mint_account: pool_authority_mint_account_pda,
            pool_authority_wsol_account: pool_authority_wsol_account_pda,
            amm_global_config: amm_global_config_pda,
            lp_mint: lp_mint_pda,
            user_pool_token_account: user_pool_token_account_pda,
            pool_base_token_account: pool_base_token_account_pda,
            pool_quote_token_account: pool_quote_token_account_pda,
        },
    )
    .signer(&user)
    .log()
    .send_and_confirm()
    .expect("calling migrate again on an already-migrated curve should succeed as a real no-op, not error");

    let pool_base_balance_after_second_migrate = u64::from_le_bytes(
        provider.get_account_data(&pool_base_token_account_pda).expect("pool_base_token_account should still exist")[64..72]
            .try_into()
            .unwrap(),
    );
    assert_eq!(
        pool_base_balance_after_second_migrate, pool_base_balance,
        "a second migrate call must be a real no-op -- pool_base_token_account's balance should not change"
    );
}

/// Real end-to-end `migrate_v2`: graduates a bonding curve via a real
/// `buy_v2` call (not synthetic account injection — the curve's real state
/// after a real trade), enables migration via `set_params`, then calls
/// `migrate_v2` and checks the confirmed mechanism
/// (`docs/plan/bonding-curve-05-batch1-v2-instructions.md`): the curve's
/// full remaining base balance lands in `pool_base_token_account`, the full
/// `real_quote_reserves` lands in `pool_authority_quote_account` then
/// `pool_quote_token_account` via `create_pool`, `init_boost` then moves
/// `floor(quote_balance * base_balance / BOOST_BASE_SUPPLY_DIVISOR)` of that
/// out into a freshly-created `boost_vault` and records it as
/// `pool.virtual_quote_reserves`, the bonding curve's reserves are zeroed,
/// and the migrated pool's LP-token account no longer exists (burned +
/// closed).
#[test]
fn migrate_v2_moves_bonding_curve_reserves_into_a_new_pump_amm_pool_with_boost() {
    let provider = setup();
    load_pump_amm_program(&provider);
    let (
        global_pda,
        bonding_curve_pda,
        mint,
        bonding_curve_bump,
        creator,
        fee_recipient_accounts,
        buyback_vaults,
        buyback_bumps,
        authority,
    ) = setup_tradeable_bonding_curve(&provider);

    let wsol_mint = wsol_mint_address();
    provider
        .set_account(&wsol_mint, spl_mint_account_data(None, 9), &TOKEN_PROGRAM_ID, 10_000_000)
        .expect("inject wsol_mint fixture");

    let user = Keypair::new();
    provider.airdrop(&user.address(), 200_000_000_000).unwrap();
    create_ata(&provider, &mint.address(), &user.address()).expect("create user base ATA");
    create_ata(&provider, &wsol_mint, &user.address()).expect("create user quote ATA");

    let (associated_base_bonding_curve_pda, associated_base_bonding_curve_bump) = Address::find_program_address(
        &[bonding_curve_pda.as_ref(), TOKEN_PROGRAM_ID.as_ref(), mint.address().as_ref()],
        &ASSOCIATED_TOKEN_PROGRAM_ID,
    );
    create_ata(&provider, &wsol_mint, &bonding_curve_pda).expect("create bonding_curve quote ATA");
    let (associated_quote_bonding_curve_pda, associated_quote_bonding_curve_bump) = Address::find_program_address(
        &[bonding_curve_pda.as_ref(), TOKEN_PROGRAM_ID.as_ref(), wsol_mint.as_ref()],
        &ASSOCIATED_TOKEN_PROGRAM_ID,
    );
    let (associated_base_user_pda, associated_base_user_bump) = Address::find_program_address(
        &[user.address().as_ref(), TOKEN_PROGRAM_ID.as_ref(), mint.address().as_ref()],
        &ASSOCIATED_TOKEN_PROGRAM_ID,
    );
    let (associated_quote_user_pda, associated_quote_user_bump) = Address::find_program_address(
        &[user.address().as_ref(), TOKEN_PROGRAM_ID.as_ref(), wsol_mint.as_ref()],
        &ASSOCIATED_TOKEN_PROGRAM_ID,
    );
    let (creator_vault_pda, creator_vault_bump) = Address::find_program_address(
        &[CREATOR_VAULT_SEED, creator.as_ref()],
        &PROGRAM_ID,
    );
    let (associated_creator_vault_pda, associated_creator_vault_bump) = Address::find_program_address(
        &[creator_vault_pda.as_ref(), TOKEN_PROGRAM_ID.as_ref(), wsol_mint.as_ref()],
        &ASSOCIATED_TOKEN_PROGRAM_ID,
    );
    let (associated_quote_fee_recipient_pda, associated_quote_fee_recipient_bump) = Address::find_program_address(
        &[fee_recipient_accounts[0].as_ref(), TOKEN_PROGRAM_ID.as_ref(), wsol_mint.as_ref()],
        &ASSOCIATED_TOKEN_PROGRAM_ID,
    );
    create_ata(&provider, &wsol_mint, &buyback_vaults[0]).expect("create buyback quote ATA");
    let (associated_quote_buyback_fee_recipient_pda, associated_quote_buyback_fee_recipient_bump) =
        Address::find_program_address(
            &[buyback_vaults[0].as_ref(), TOKEN_PROGRAM_ID.as_ref(), wsol_mint.as_ref()],
            &ASSOCIATED_TOKEN_PROGRAM_ID,
        );
    let (global_volume_accumulator_pda, _) =
        Address::find_program_address(&[GLOBAL_VOLUME_ACCUMULATOR_SEED], &PROGRAM_ID);
    provider
        .set_account(&global_volume_accumulator_pda, global_volume_accumulator_account_data(), &PROGRAM_ID, 10_000_000)
        .expect("inject global_volume_accumulator fixture");
    let (user_volume_accumulator_pda, user_volume_accumulator_bump) = Address::find_program_address(
        &[USER_VOLUME_ACCUMULATOR_SEED, user.address().as_ref()],
        &PROGRAM_ID,
    );
    let (associated_user_volume_accumulator_pda, associated_user_volume_accumulator_bump) =
        Address::find_program_address(
            &[user_volume_accumulator_pda.as_ref(), TOKEN_PROGRAM_ID.as_ref(), wsol_mint.as_ref()],
            &ASSOCIATED_TOKEN_PROGRAM_ID,
        );
    let (fee_config_pda, fee_config_bump) = Address::find_program_address(
        &[FEE_CONFIG_SEED, PROGRAM_ID.as_ref()],
        &PUMP_FEES_PROGRAM_ID,
    );
    let (pump_authority_pda, _) = Address::find_program_address(&[PUMP_AUTHORITY_SEED], &PROGRAM_ID);

    // Real pump-fun graduation economics: buy the curve's entire
    // `real_token_reserves` in one `buy_v2` trade (SOL-quote path), same
    // trigger classic `buy`'s own graduation test uses.
    let full_amount = 793_100_000_000_000u64;
    build_buy_v2(
        &provider,
        PROGRAM_ID,
        BuyV2Args {
            amount: full_amount,
            max_sol_cost: 150_000_000_000,
            bonding_curve_bump,
            associated_base_bonding_curve_bump,
            associated_quote_bonding_curve_bump,
            associated_base_user_bump,
            associated_quote_user_bump,
            creator_vault_bump,
            associated_creator_vault_bump,
            associated_quote_fee_recipient_bump,
            associated_quote_buyback_fee_recipient_bump,
            user_volume_accumulator_bump,
            associated_user_volume_accumulator_bump,
            fee_config_bump,
            buyback_index: 0,
            buyback_vault_bump: buyback_bumps[0],
        },
        BuyV2Accounts {
            global: global_pda,
            base_mint: mint.address(),
            quote_mint: wsol_mint,
            base_token_program: TOKEN_PROGRAM_ID,
            quote_token_program: TOKEN_PROGRAM_ID,
            associated_token_program: ASSOCIATED_TOKEN_PROGRAM_ID,
            fee_recipient: fee_recipient_accounts[0],
            associated_quote_fee_recipient: associated_quote_fee_recipient_pda,
            buyback_fee_recipient: buyback_vaults[0],
            associated_quote_buyback_fee_recipient: associated_quote_buyback_fee_recipient_pda,
            bonding_curve: bonding_curve_pda,
            associated_base_bonding_curve: associated_base_bonding_curve_pda,
            associated_quote_bonding_curve: associated_quote_bonding_curve_pda,
            user: user.address(),
            associated_base_user: associated_base_user_pda,
            associated_quote_user: associated_quote_user_pda,
            creator_vault: creator_vault_pda,
            associated_creator_vault: associated_creator_vault_pda,
            sharing_config: Address::default(),
            global_volume_accumulator: global_volume_accumulator_pda,
            user_volume_accumulator: user_volume_accumulator_pda,
            associated_user_volume_accumulator: associated_user_volume_accumulator_pda,
            program: PROGRAM_ID,
            fee_config: fee_config_pda,
            fee_program: PUMP_FEES_PROGRAM_ID,
            system_program: SYSTEM_PROGRAM_ID,
            pump_authority: pump_authority_pda,
        },
    )
    .signer(&user)
    .send_and_confirm()
    .expect("graduating buy_v2 should succeed");

    let graduated = fetch_bonding_curve(&provider, &bonding_curve_pda)
        .expect("bonding_curve should be readable");
    assert!(bool::from(graduated.complete), "bonding curve should be complete after buying out its full reserves");
    assert_eq!(graduated.real_token_reserves, 0);
    let real_quote_reserves = graduated.real_quote_reserves;

    let withdraw_authority = Keypair::new().address();
    let pool_migration_fee: u64 = 30_000_000;
    build_set_params(
        &provider,
        PROGRAM_ID,
        pump_client::SetParamsArgs {
            initial_virtual_token_reserves: 1_073_000_000_000_000,
            initial_virtual_sol_reserves: 30_000_000_000,
            initial_real_token_reserves: 793_100_000_000_000,
            token_total_supply: 1_000_000_000_000_000,
            fee_basis_points: 0,
            withdraw_authority,
            enable_migrate: Bool::from(true),
            pool_migration_fee,
            creator_fee_basis_points: 0,
            set_creator_authority: Address::default(),
            admin_set_creator_authority: Address::default(),
        },
        SetParamsAccounts { global: global_pda, authority: authority.address() },
    )
    .signer(&authority)
    .remaining_accounts(
        fee_recipient_accounts
            .iter()
            .map(|address| AccountMeta { address: *address, is_signer: false, is_writable: false })
            .collect(),
    )
    .send_and_confirm()
    .expect("set_params should succeed enabling migrate");

    // `migrate_v2` unconditionally CPIs into `init_boost`, which requires
    // `boost_enabled = true` — unlike classic `migrate`'s fixture, which
    // leaves boost off since it never touches it.
    let (amm_global_config_pda, amm_global_config_bump) =
        Address::find_program_address(&[GLOBAL_CONFIG_SEED], &PUMP_AMM_PROGRAM_ID);
    let mut amm_global_config_data = vec![149, 8, 156, 202, 160, 252, 176, 217];
    amm_global_config_data.push(0); // bump — unused by `create_pool`/`init_boost`
    amm_global_config_data.push(0); // disable_flags
    amm_global_config_data.push(1); // boost_enabled = true
    amm_global_config_data.extend_from_slice(&[0u8; 32]); // admin
    amm_global_config_data.extend_from_slice(&[0u8; 32]); // boost_authority
    provider
        .set_account(&amm_global_config_pda, amm_global_config_data, &PUMP_AMM_PROGRAM_ID, 10_000_000)
        .expect("inject amm_global_config fixture");

    let (pool_authority_pda, pool_authority_bump) =
        Address::find_program_address(&[POOL_AUTHORITY_SEED, mint.address().as_ref()], &PROGRAM_ID);
    let (pool_pda, pool_bump) = Address::find_program_address(
        &[POOL_SEED, &0u16.to_le_bytes(), pool_authority_pda.as_ref(), mint.address().as_ref(), wsol_mint.as_ref()],
        &PUMP_AMM_PROGRAM_ID,
    );
    let (pool_authority_mint_account_pda, pool_authority_mint_account_bump) = Address::find_program_address(
        &[pool_authority_pda.as_ref(), TOKEN_PROGRAM_ID.as_ref(), mint.address().as_ref()],
        &ASSOCIATED_TOKEN_PROGRAM_ID,
    );
    let (pool_authority_quote_account_pda, pool_authority_quote_account_bump) = Address::find_program_address(
        &[pool_authority_pda.as_ref(), TOKEN_PROGRAM_ID.as_ref(), wsol_mint.as_ref()],
        &ASSOCIATED_TOKEN_PROGRAM_ID,
    );
    let (lp_mint_pda, lp_mint_bump) =
        Address::find_program_address(&[POOL_LP_MINT_SEED, pool_pda.as_ref()], &PUMP_AMM_PROGRAM_ID);
    let (user_pool_token_account_pda, user_pool_token_account_bump) = Address::find_program_address(
        &[pool_authority_pda.as_ref(), TOKEN_2022_PROGRAM_ID.as_ref(), lp_mint_pda.as_ref()],
        &ASSOCIATED_TOKEN_PROGRAM_ID,
    );
    let (pool_base_token_account_pda, pool_base_token_account_bump) = Address::find_program_address(
        &[pool_pda.as_ref(), TOKEN_PROGRAM_ID.as_ref(), mint.address().as_ref()],
        &ASSOCIATED_TOKEN_PROGRAM_ID,
    );
    let (pool_quote_token_account_pda, pool_quote_token_account_bump) = Address::find_program_address(
        &[pool_pda.as_ref(), TOKEN_PROGRAM_ID.as_ref(), wsol_mint.as_ref()],
        &ASSOCIATED_TOKEN_PROGRAM_ID,
    );
    let (boost_vault_authority_pda, boost_vault_authority_bump) =
        Address::find_program_address(&[BOOST_VAULT_SEED, pool_pda.as_ref()], &PUMP_AMM_PROGRAM_ID);
    let (boost_vault_pda, boost_vault_bump) = Address::find_program_address(
        &[boost_vault_authority_pda.as_ref(), TOKEN_PROGRAM_ID.as_ref(), wsol_mint.as_ref()],
        &ASSOCIATED_TOKEN_PROGRAM_ID,
    );

    build_migrate_v2(
        &provider,
        PROGRAM_ID,
        MigrateV2Args {
            bonding_curve_bump,
            associated_base_bonding_curve_bump,
            associated_quote_bonding_curve_bump,
            pool_authority_bump,
            pool_authority_mint_account_bump,
            pool_authority_quote_account_bump,
            amm_global_config_bump,
            pool_bump,
            lp_mint_bump,
            user_pool_token_account_bump,
            pool_base_token_account_bump,
            pool_quote_token_account_bump,
            boost_vault_authority_bump,
            boost_vault_bump,
        },
        MigrateV2Accounts {
            global: global_pda,
            withdraw_authority,
            base_mint: mint.address(),
            quote_mint: wsol_mint,
            bonding_curve: bonding_curve_pda,
            associated_base_bonding_curve: associated_base_bonding_curve_pda,
            associated_quote_bonding_curve: associated_quote_bonding_curve_pda,
            user: user.address(),
            system_program: SYSTEM_PROGRAM_ID,
            base_token_program: TOKEN_PROGRAM_ID,
            quote_token_program: TOKEN_PROGRAM_ID,
            token_2022_program: TOKEN_2022_PROGRAM_ID,
            associated_token_program: ASSOCIATED_TOKEN_PROGRAM_ID,
            pump_amm: PUMP_AMM_PROGRAM_ID,
            rent: RENT_SYSVAR_ID,
            pool_authority: pool_authority_pda,
            pool: pool_pda,
            pool_authority_mint_account: pool_authority_mint_account_pda,
            pool_authority_quote_account: pool_authority_quote_account_pda,
            amm_global_config: amm_global_config_pda,
            lp_mint: lp_mint_pda,
            user_pool_token_account: user_pool_token_account_pda,
            pool_base_token_account: pool_base_token_account_pda,
            pool_quote_token_account: pool_quote_token_account_pda,
            boost_vault_authority: boost_vault_authority_pda,
            boost_vault: boost_vault_pda,
        },
    )
    .signer(&user)
    .log()
    .send_and_confirm()
    .expect("migrate_v2 should succeed");

    let pool_base_balance = u64::from_le_bytes(
        provider.get_account_data(&pool_base_token_account_pda).expect("pool_base_token_account should exist")[64..72]
            .try_into()
            .unwrap(),
    );
    let token_total_supply = 1_000_000_000_000_000u64;
    assert_eq!(
        pool_base_balance,
        token_total_supply - full_amount,
        "associated_base_bonding_curve's full remaining token balance should land in pool_base_token_account"
    );

    let boost_vault_balance = u64::from_le_bytes(
        provider.get_account_data(&boost_vault_pda).expect("boost_vault should exist")[64..72].try_into().unwrap(),
    );
    let expected_virtual_quote_reserves =
        ((real_quote_reserves as u128) * (pool_base_balance as u128) / 1_000_000_000_000_000u128) as u64;
    assert_eq!(
        boost_vault_balance,
        expected_virtual_quote_reserves,
        "boost_vault should hold the confirmed init_boost split formula's output"
    );

    let pool_quote_balance = u64::from_le_bytes(
        provider.get_account_data(&pool_quote_token_account_pda).expect("pool_quote_token_account should exist")[64..72]
            .try_into()
            .unwrap(),
    );
    assert_eq!(
        pool_quote_balance,
        real_quote_reserves - expected_virtual_quote_reserves,
        "pool_quote_token_account should hold real_quote_reserves minus the amount moved into boost_vault"
    );

    let migrated = fetch_bonding_curve(&provider, &bonding_curve_pda)
        .expect("bonding_curve should still be readable after migrate_v2");
    assert_eq!(migrated.real_token_reserves, 0);
    assert_eq!(migrated.real_quote_reserves, 0);
    assert_eq!(migrated.virtual_token_reserves, 0);
    assert_eq!(migrated.virtual_quote_reserves, 0);

    assert!(
        provider.get_account_data(&user_pool_token_account_pda).is_err(),
        "pool_authority's LP-token account should have been burned and closed"
    );

    // Real, confirmed via `reference/fee-tier-probe/src/bin/probe56.rs`
    // through `probe63.rs`, independently corroborated by real
    // `MigrateV2` mainnet transactions logging the identical line: calling
    // `migrate_v2` again on an already-migrated curve succeeds as a real
    // no-op rather than erroring.
    provider.expire_blockhash().unwrap();
    build_migrate_v2(
        &provider,
        PROGRAM_ID,
        MigrateV2Args {
            bonding_curve_bump,
            associated_base_bonding_curve_bump,
            associated_quote_bonding_curve_bump,
            pool_authority_bump,
            pool_authority_mint_account_bump,
            pool_authority_quote_account_bump,
            amm_global_config_bump,
            pool_bump,
            lp_mint_bump,
            user_pool_token_account_bump,
            pool_base_token_account_bump,
            pool_quote_token_account_bump,
            boost_vault_authority_bump,
            boost_vault_bump,
        },
        MigrateV2Accounts {
            global: global_pda,
            withdraw_authority,
            base_mint: mint.address(),
            quote_mint: wsol_mint,
            bonding_curve: bonding_curve_pda,
            associated_base_bonding_curve: associated_base_bonding_curve_pda,
            associated_quote_bonding_curve: associated_quote_bonding_curve_pda,
            user: user.address(),
            system_program: SYSTEM_PROGRAM_ID,
            base_token_program: TOKEN_PROGRAM_ID,
            quote_token_program: TOKEN_PROGRAM_ID,
            token_2022_program: TOKEN_2022_PROGRAM_ID,
            associated_token_program: ASSOCIATED_TOKEN_PROGRAM_ID,
            pump_amm: PUMP_AMM_PROGRAM_ID,
            rent: RENT_SYSVAR_ID,
            pool_authority: pool_authority_pda,
            pool: pool_pda,
            pool_authority_mint_account: pool_authority_mint_account_pda,
            pool_authority_quote_account: pool_authority_quote_account_pda,
            amm_global_config: amm_global_config_pda,
            lp_mint: lp_mint_pda,
            user_pool_token_account: user_pool_token_account_pda,
            pool_base_token_account: pool_base_token_account_pda,
            pool_quote_token_account: pool_quote_token_account_pda,
            boost_vault_authority: boost_vault_authority_pda,
            boost_vault: boost_vault_pda,
        },
    )
    .signer(&user)
    .log()
    .send_and_confirm()
    .expect("calling migrate_v2 again on an already-migrated curve should succeed as a real no-op, not error");

    let pool_base_balance_after_second_migrate = u64::from_le_bytes(
        provider.get_account_data(&pool_base_token_account_pda).expect("pool_base_token_account should still exist")[64..72]
            .try_into()
            .unwrap(),
    );
    assert_eq!(
        pool_base_balance_after_second_migrate, pool_base_balance,
        "a second migrate_v2 call must be a real no-op -- pool_base_token_account's balance should not change"
    );
}

/// Same graduation-and-migrate flow as
/// `migrate_v2_moves_bonding_curve_reserves_into_a_new_pump_amm_pool_with_boost`,
/// but against a `create_v2` pool paired with a whitelisted mimic-SOL mint
/// instead of WSOL -- proves the whole `add_quote_mint` -> quote-mint-paired
/// `create_v2` -> `buy_v2` -> `migrate_v2` chain works end to end on a real
/// (non-SOL) quote mint, not just that each piece works in isolation. This
/// is the actual point of building `add_quote_mint`/`set_virtual_quote_reserves`
/// in the first place: real graduation testing without spending real SOL.
#[test]
fn migrate_v2_moves_bonding_curve_reserves_with_a_whitelisted_quote_mint() {
    let provider = setup();
    load_pump_amm_program(&provider);
    let (
        global_pda,
        _bonding_curve_pda,
        _mint,
        _bonding_curve_bump,
        creator,
        fee_recipient_accounts,
        buyback_vaults,
        buyback_bumps,
        authority,
    ) = setup_tradeable_bonding_curve(&provider);

    // Mimic-SOL: a freshly minted, 9-decimal classic SPL Token mint, whitelisted
    // for this test only (litesvm gets a fresh VM per run, so no cleanup needed).
    let quote_mint_kp = Keypair::new();
    create_mint(&provider, &quote_mint_kp, &provider.payer.address(), 9)
        .expect("create mimic-SOL quote mint should succeed");
    let quote_mint = quote_mint_kp.address();

    build_add_quote_mint(
        &provider,
        PROGRAM_ID,
        quote_mint,
        AddQuoteMintAccounts { global: global_pda, authority: authority.address() },
    )
    .signer(&authority)
    .send_and_confirm()
    .expect("add_quote_mint should succeed");

    // Real value, not a guess: `Global.initial_virtual_quote_reserves` fetched
    // directly off the real mainnet `Global` account, confirmed via
    // `reference/fee-tier-probe/src/bin/probe64.rs`'s own printed output.
    const REAL_INITIAL_VIRTUAL_QUOTE_RESERVES: u64 = 4_292_000_000;
    build_set_virtual_quote_reserves(
        &provider,
        PROGRAM_ID,
        REAL_INITIAL_VIRTUAL_QUOTE_RESERVES,
        SetVirtualQuoteReservesAccounts { global: global_pda, authority: authority.address() },
    )
    .signer(&authority)
    .send_and_confirm()
    .expect("set_virtual_quote_reserves should succeed");

    let (result, bonding_curve_pda, mint) =
        create_bonding_curve_v2_with_quote_mint(&provider, global_pda, creator, Some(quote_mint));
    result.expect("create_v2 with a whitelisted quote mint should succeed");
    let (_, bonding_curve_bump) = get_bonding_curve_pda(&PROGRAM_ID, &mint.address());

    let user = Keypair::new();
    provider.airdrop(&user.address(), 200_000_000_000).unwrap();
    // Base mint is Token-2022 (`create_v2` always mints Token-2022) --
    // `create_ata` defaults to classic Token, so the base ATA needs the
    // explicit `_with_program` variant.
    create_ata_with_program(&provider, &mint.address(), &user.address(), &TOKEN_2022_PROGRAM_ID)
        .expect("create user base ATA");
    let user_quote_ata =
        create_ata(&provider, &quote_mint, &user.address()).expect("create user quote ATA");
    // Real graduation cost for this curve (`ceil(793_100_000_000_000 *
    // 4_292_000_000 / (1_073_000_000_000_000 - 793_100_000_000_000))` ~=
    // 12.16e9) plus ~1.5% protocol/creator fee headroom -- minted generously
    // above that, not tightly, since this is a test fixture, not a real
    // economic constraint.
    mint_to(&provider, &quote_mint, &user_quote_ata, &provider.payer, 20_000_000_000)
        .expect("mint mimic-SOL supply to trading user should succeed");

    let (associated_base_bonding_curve_pda, associated_base_bonding_curve_bump) = Address::find_program_address(
        &[bonding_curve_pda.as_ref(), TOKEN_2022_PROGRAM_ID.as_ref(), mint.address().as_ref()],
        &ASSOCIATED_TOKEN_PROGRAM_ID,
    );
    let (associated_quote_bonding_curve_pda, associated_quote_bonding_curve_bump) = Address::find_program_address(
        &[bonding_curve_pda.as_ref(), TOKEN_PROGRAM_ID.as_ref(), quote_mint.as_ref()],
        &ASSOCIATED_TOKEN_PROGRAM_ID,
    );
    let (associated_base_user_pda, associated_base_user_bump) = Address::find_program_address(
        &[user.address().as_ref(), TOKEN_2022_PROGRAM_ID.as_ref(), mint.address().as_ref()],
        &ASSOCIATED_TOKEN_PROGRAM_ID,
    );
    let (associated_quote_user_pda, associated_quote_user_bump) = Address::find_program_address(
        &[user.address().as_ref(), TOKEN_PROGRAM_ID.as_ref(), quote_mint.as_ref()],
        &ASSOCIATED_TOKEN_PROGRAM_ID,
    );
    let (creator_vault_pda, creator_vault_bump) = Address::find_program_address(
        &[CREATOR_VAULT_SEED, creator.as_ref()],
        &PROGRAM_ID,
    );
    let (associated_creator_vault_pda, associated_creator_vault_bump) = Address::find_program_address(
        &[creator_vault_pda.as_ref(), TOKEN_PROGRAM_ID.as_ref(), quote_mint.as_ref()],
        &ASSOCIATED_TOKEN_PROGRAM_ID,
    );
    let (associated_quote_fee_recipient_pda, associated_quote_fee_recipient_bump) = Address::find_program_address(
        &[fee_recipient_accounts[0].as_ref(), TOKEN_PROGRAM_ID.as_ref(), quote_mint.as_ref()],
        &ASSOCIATED_TOKEN_PROGRAM_ID,
    );
    create_ata(&provider, &quote_mint, &buyback_vaults[0]).expect("create buyback quote ATA");
    let (associated_quote_buyback_fee_recipient_pda, associated_quote_buyback_fee_recipient_bump) =
        Address::find_program_address(
            &[buyback_vaults[0].as_ref(), TOKEN_PROGRAM_ID.as_ref(), quote_mint.as_ref()],
            &ASSOCIATED_TOKEN_PROGRAM_ID,
        );
    let (global_volume_accumulator_pda, _) =
        Address::find_program_address(&[GLOBAL_VOLUME_ACCUMULATOR_SEED], &PROGRAM_ID);
    provider
        .set_account(&global_volume_accumulator_pda, global_volume_accumulator_account_data(), &PROGRAM_ID, 10_000_000)
        .expect("inject global_volume_accumulator fixture");
    let (user_volume_accumulator_pda, user_volume_accumulator_bump) = Address::find_program_address(
        &[USER_VOLUME_ACCUMULATOR_SEED, user.address().as_ref()],
        &PROGRAM_ID,
    );
    let (associated_user_volume_accumulator_pda, associated_user_volume_accumulator_bump) =
        Address::find_program_address(
            &[user_volume_accumulator_pda.as_ref(), TOKEN_PROGRAM_ID.as_ref(), quote_mint.as_ref()],
            &ASSOCIATED_TOKEN_PROGRAM_ID,
        );
    let (fee_config_pda, fee_config_bump) = Address::find_program_address(
        &[FEE_CONFIG_SEED, PROGRAM_ID.as_ref()],
        &PUMP_FEES_PROGRAM_ID,
    );
    let (pump_authority_pda, _) = Address::find_program_address(&[PUMP_AUTHORITY_SEED], &PROGRAM_ID);

    // Same real graduation trigger as the WSOL-paired test: buy the curve's
    // entire `real_token_reserves` in one `buy_v2` trade.
    let full_amount = 793_100_000_000_000u64;

    // Exact-value fee assertions on the real non-SOL-quote SPL-transfer path
    // (`buy_v2.rs`'s `else` branch) -- no existing test in this file
    // exercised this path with real balance checks before now (the SOL-quote
    // `buy_v2` test explicitly asserts every quote-side ATA stays untouched,
    // since it uses native lamports instead). Mirrors
    // `buy_v2_purchases_tokens_and_updates_reserves`'s own exact-value
    // pattern, adapted to the SPL destinations `buy_v2.rs` actually credits
    // for a non-SOL quote: `associated_quote_bonding_curve`/
    // `associated_quote_fee_recipient`/`associated_quote_buyback_fee_recipient`/
    // `associated_creator_vault`, not native lamports.
    let bonding_curve_before_buy =
        fetch_bonding_curve(&provider, &bonding_curve_pda).expect("bonding_curve should be readable");
    let expected_net_quote = expected_gross_buy(
        full_amount as u128,
        bonding_curve_before_buy.virtual_quote_reserves as u128,
        bonding_curve_before_buy.virtual_token_reserves as u128,
    );
    // Uses the *stable* bps (150/75), not `TEST_PROTOCOL_FEE_BPS`/
    // `TEST_CREATOR_FEE_BPS` (100/50) -- this quote mint is non-SOL, so
    // `get_fees` must read `stable_fee_tiers`, not `fee_tiers`. Asserting
    // against the stable-specific values (deliberately different from the
    // classic ones) is what actually proves the correct table was read,
    // rather than passing regardless because both tables happened to agree.
    let expected_protocol_fee = expected_fee_ceil(expected_net_quote as u128, TEST_STABLE_PROTOCOL_FEE_BPS);
    let expected_creator_fee = expected_fee_ceil(expected_net_quote as u128, TEST_STABLE_CREATOR_FEE_BPS);
    let expected_buyback_share = expected_fee_floor(expected_protocol_fee as u128, TEST_BUYBACK_BASIS_POINTS);
    let expected_fee_recipient_share = expected_protocol_fee - expected_buyback_share;
    let expected_total_cost = expected_net_quote + expected_protocol_fee + expected_creator_fee;

    // `associated_quote_bonding_curve` already exists (created by `create_v2`
    // above); `associated_quote_fee_recipient`/`associated_creator_vault` are
    // `init_if_needed` in `buy_v2.rs` and don't exist yet, so their "before"
    // balance is implicitly 0 -- read only the ones that already exist.
    let associated_quote_bonding_curve_before = token_balance(&provider, &associated_quote_bonding_curve_pda);
    let associated_quote_user_before = token_balance(&provider, &associated_quote_user_pda);
    let associated_quote_buyback_fee_recipient_before =
        token_balance(&provider, &associated_quote_buyback_fee_recipient_pda);
    let creator_vault_lamports_before = provider.get_balance(&creator_vault_pda).unwrap();

    build_buy_v2(
        &provider,
        PROGRAM_ID,
        BuyV2Args {
            amount: full_amount,
            max_sol_cost: 20_000_000_000,
            bonding_curve_bump,
            associated_base_bonding_curve_bump,
            associated_quote_bonding_curve_bump,
            associated_base_user_bump,
            associated_quote_user_bump,
            creator_vault_bump,
            associated_creator_vault_bump,
            associated_quote_fee_recipient_bump,
            associated_quote_buyback_fee_recipient_bump,
            user_volume_accumulator_bump,
            associated_user_volume_accumulator_bump,
            fee_config_bump,
            buyback_index: 0,
            buyback_vault_bump: buyback_bumps[0],
        },
        BuyV2Accounts {
            global: global_pda,
            base_mint: mint.address(),
            quote_mint,
            base_token_program: TOKEN_2022_PROGRAM_ID,
            quote_token_program: TOKEN_PROGRAM_ID,
            associated_token_program: ASSOCIATED_TOKEN_PROGRAM_ID,
            fee_recipient: fee_recipient_accounts[0],
            associated_quote_fee_recipient: associated_quote_fee_recipient_pda,
            buyback_fee_recipient: buyback_vaults[0],
            associated_quote_buyback_fee_recipient: associated_quote_buyback_fee_recipient_pda,
            bonding_curve: bonding_curve_pda,
            associated_base_bonding_curve: associated_base_bonding_curve_pda,
            associated_quote_bonding_curve: associated_quote_bonding_curve_pda,
            user: user.address(),
            associated_base_user: associated_base_user_pda,
            associated_quote_user: associated_quote_user_pda,
            creator_vault: creator_vault_pda,
            associated_creator_vault: associated_creator_vault_pda,
            sharing_config: Address::default(),
            global_volume_accumulator: global_volume_accumulator_pda,
            user_volume_accumulator: user_volume_accumulator_pda,
            associated_user_volume_accumulator: associated_user_volume_accumulator_pda,
            program: PROGRAM_ID,
            fee_config: fee_config_pda,
            fee_program: PUMP_FEES_PROGRAM_ID,
            system_program: SYSTEM_PROGRAM_ID,
            pump_authority: pump_authority_pda,
        },
    )
    .signer(&user)
    .log()
    .send_and_confirm()
    .expect("graduating buy_v2 against the mimic-SOL quote mint should succeed");

    assert_eq!(
        token_balance(&provider, &associated_quote_bonding_curve_pda),
        associated_quote_bonding_curve_before + expected_net_quote,
        "associated_quote_bonding_curve should be credited exactly net_quote"
    );
    assert_eq!(
        token_balance(&provider, &associated_quote_user_pda),
        associated_quote_user_before - expected_total_cost,
        "associated_quote_user should be debited exactly net_quote + protocol_fee + creator_fee"
    );
    assert_eq!(
        token_balance(&provider, &associated_quote_fee_recipient_pda),
        expected_fee_recipient_share,
        "associated_quote_fee_recipient (freshly created via init_if_needed) should receive exactly protocol_fee minus the buyback carve-out"
    );
    assert_eq!(
        token_balance(&provider, &associated_quote_buyback_fee_recipient_pda),
        associated_quote_buyback_fee_recipient_before + expected_buyback_share,
        "associated_quote_buyback_fee_recipient should receive exactly the buyback carve-out (0 in this fixture)"
    );
    assert_eq!(
        token_balance(&provider, &associated_creator_vault_pda),
        expected_creator_fee,
        "associated_creator_vault (freshly created via init_if_needed) should receive exactly creator_fee -- non-SOL quote routes the fee here, not to creator_vault's native lamports"
    );
    let creator_vault_lamports_after = provider.get_balance(&creator_vault_pda).unwrap();
    assert_eq!(
        creator_vault_lamports_after,
        creator_vault_lamports_before + CREATOR_VAULT_RENT_EXEMPT_MINIMUM,
        "creator_vault's native lamports should only receive the rent-exempt top-up for a non-SOL quote, since creator_fee itself lands in associated_creator_vault"
    );
    assert_eq!(
        token_balance(&provider, &associated_base_user_pda),
        full_amount,
        "user should have received exactly the purchased base-token amount"
    );

    let graduated = fetch_bonding_curve(&provider, &bonding_curve_pda)
        .expect("bonding_curve should be readable");
    assert!(bool::from(graduated.complete), "bonding curve should be complete after buying out its full reserves");
    assert_eq!(graduated.real_token_reserves, 0);
    let real_quote_reserves = graduated.real_quote_reserves;

    let withdraw_authority = Keypair::new().address();
    let pool_migration_fee: u64 = 30_000_000;
    build_set_params(
        &provider,
        PROGRAM_ID,
        pump_client::SetParamsArgs {
            initial_virtual_token_reserves: 1_073_000_000_000_000,
            initial_virtual_sol_reserves: 30_000_000_000,
            initial_real_token_reserves: 793_100_000_000_000,
            token_total_supply: 1_000_000_000_000_000,
            fee_basis_points: 0,
            withdraw_authority,
            enable_migrate: Bool::from(true),
            pool_migration_fee,
            creator_fee_basis_points: 0,
            set_creator_authority: Address::default(),
            admin_set_creator_authority: Address::default(),
        },
        SetParamsAccounts { global: global_pda, authority: authority.address() },
    )
    .signer(&authority)
    .remaining_accounts(
        fee_recipient_accounts
            .iter()
            .map(|address| AccountMeta { address: *address, is_signer: false, is_writable: false })
            .collect(),
    )
    .send_and_confirm()
    .expect("set_params should succeed enabling migrate");

    // `migrate_v2` unconditionally CPIs into `init_boost`, which requires
    // `boost_enabled = true` -- same fixture the WSOL-paired test injects.
    let (amm_global_config_pda, amm_global_config_bump) =
        Address::find_program_address(&[GLOBAL_CONFIG_SEED], &PUMP_AMM_PROGRAM_ID);
    let mut amm_global_config_data = vec![149, 8, 156, 202, 160, 252, 176, 217];
    amm_global_config_data.push(0); // bump -- unused by `create_pool`/`init_boost`
    amm_global_config_data.push(0); // disable_flags
    amm_global_config_data.push(1); // boost_enabled = true
    amm_global_config_data.extend_from_slice(&[0u8; 32]); // admin
    amm_global_config_data.extend_from_slice(&[0u8; 32]); // boost_authority
    provider
        .set_account(&amm_global_config_pda, amm_global_config_data, &PUMP_AMM_PROGRAM_ID, 10_000_000)
        .expect("inject amm_global_config fixture");

    let (pool_authority_pda, pool_authority_bump) =
        Address::find_program_address(&[POOL_AUTHORITY_SEED, mint.address().as_ref()], &PROGRAM_ID);
    let (pool_pda, pool_bump) = Address::find_program_address(
        &[POOL_SEED, &0u16.to_le_bytes(), pool_authority_pda.as_ref(), mint.address().as_ref(), quote_mint.as_ref()],
        &PUMP_AMM_PROGRAM_ID,
    );
    let (pool_authority_mint_account_pda, pool_authority_mint_account_bump) = Address::find_program_address(
        &[pool_authority_pda.as_ref(), TOKEN_2022_PROGRAM_ID.as_ref(), mint.address().as_ref()],
        &ASSOCIATED_TOKEN_PROGRAM_ID,
    );
    let (pool_authority_quote_account_pda, pool_authority_quote_account_bump) = Address::find_program_address(
        &[pool_authority_pda.as_ref(), TOKEN_PROGRAM_ID.as_ref(), quote_mint.as_ref()],
        &ASSOCIATED_TOKEN_PROGRAM_ID,
    );
    let (lp_mint_pda, lp_mint_bump) =
        Address::find_program_address(&[POOL_LP_MINT_SEED, pool_pda.as_ref()], &PUMP_AMM_PROGRAM_ID);
    let (user_pool_token_account_pda, user_pool_token_account_bump) = Address::find_program_address(
        &[pool_authority_pda.as_ref(), TOKEN_2022_PROGRAM_ID.as_ref(), lp_mint_pda.as_ref()],
        &ASSOCIATED_TOKEN_PROGRAM_ID,
    );
    let (pool_base_token_account_pda, pool_base_token_account_bump) = Address::find_program_address(
        &[pool_pda.as_ref(), TOKEN_2022_PROGRAM_ID.as_ref(), mint.address().as_ref()],
        &ASSOCIATED_TOKEN_PROGRAM_ID,
    );
    let (pool_quote_token_account_pda, pool_quote_token_account_bump) = Address::find_program_address(
        &[pool_pda.as_ref(), TOKEN_PROGRAM_ID.as_ref(), quote_mint.as_ref()],
        &ASSOCIATED_TOKEN_PROGRAM_ID,
    );
    let (boost_vault_authority_pda, boost_vault_authority_bump) =
        Address::find_program_address(&[BOOST_VAULT_SEED, pool_pda.as_ref()], &PUMP_AMM_PROGRAM_ID);
    let (boost_vault_pda, boost_vault_bump) = Address::find_program_address(
        &[boost_vault_authority_pda.as_ref(), TOKEN_PROGRAM_ID.as_ref(), quote_mint.as_ref()],
        &ASSOCIATED_TOKEN_PROGRAM_ID,
    );

    build_migrate_v2(
        &provider,
        PROGRAM_ID,
        MigrateV2Args {
            bonding_curve_bump,
            associated_base_bonding_curve_bump,
            associated_quote_bonding_curve_bump,
            pool_authority_bump,
            pool_authority_mint_account_bump,
            pool_authority_quote_account_bump,
            amm_global_config_bump,
            pool_bump,
            lp_mint_bump,
            user_pool_token_account_bump,
            pool_base_token_account_bump,
            pool_quote_token_account_bump,
            boost_vault_authority_bump,
            boost_vault_bump,
        },
        MigrateV2Accounts {
            global: global_pda,
            withdraw_authority,
            base_mint: mint.address(),
            quote_mint,
            bonding_curve: bonding_curve_pda,
            associated_base_bonding_curve: associated_base_bonding_curve_pda,
            associated_quote_bonding_curve: associated_quote_bonding_curve_pda,
            user: user.address(),
            system_program: SYSTEM_PROGRAM_ID,
            base_token_program: TOKEN_2022_PROGRAM_ID,
            quote_token_program: TOKEN_PROGRAM_ID,
            token_2022_program: TOKEN_2022_PROGRAM_ID,
            associated_token_program: ASSOCIATED_TOKEN_PROGRAM_ID,
            pump_amm: PUMP_AMM_PROGRAM_ID,
            rent: RENT_SYSVAR_ID,
            pool_authority: pool_authority_pda,
            pool: pool_pda,
            pool_authority_mint_account: pool_authority_mint_account_pda,
            pool_authority_quote_account: pool_authority_quote_account_pda,
            amm_global_config: amm_global_config_pda,
            lp_mint: lp_mint_pda,
            user_pool_token_account: user_pool_token_account_pda,
            pool_base_token_account: pool_base_token_account_pda,
            pool_quote_token_account: pool_quote_token_account_pda,
            boost_vault_authority: boost_vault_authority_pda,
            boost_vault: boost_vault_pda,
        },
    )
    .signer(&user)
    .log()
    .send_and_confirm()
    .expect("migrate_v2 against the mimic-SOL quote mint should succeed");

    let pool_base_balance = u64::from_le_bytes(
        provider.get_account_data(&pool_base_token_account_pda).expect("pool_base_token_account should exist")[64..72]
            .try_into()
            .unwrap(),
    );
    let token_total_supply = 1_000_000_000_000_000u64;
    assert_eq!(
        pool_base_balance,
        token_total_supply - full_amount,
        "associated_base_bonding_curve's full remaining token balance should land in pool_base_token_account"
    );

    let boost_vault_balance = u64::from_le_bytes(
        provider.get_account_data(&boost_vault_pda).expect("boost_vault should exist")[64..72].try_into().unwrap(),
    );
    let expected_virtual_quote_reserves =
        ((real_quote_reserves as u128) * (pool_base_balance as u128) / 1_000_000_000_000_000u128) as u64;
    assert_eq!(
        boost_vault_balance,
        expected_virtual_quote_reserves,
        "boost_vault should hold the confirmed init_boost split formula's output"
    );

    let pool_quote_balance = u64::from_le_bytes(
        provider.get_account_data(&pool_quote_token_account_pda).expect("pool_quote_token_account should exist")[64..72]
            .try_into()
            .unwrap(),
    );
    assert_eq!(
        pool_quote_balance,
        real_quote_reserves - expected_virtual_quote_reserves,
        "pool_quote_token_account should hold real_quote_reserves minus the amount moved into boost_vault"
    );

    let migrated = fetch_bonding_curve(&provider, &bonding_curve_pda)
        .expect("bonding_curve should still be readable after migrate_v2");
    assert_eq!(migrated.real_token_reserves, 0);
    assert_eq!(migrated.real_quote_reserves, 0);
    assert_eq!(migrated.virtual_token_reserves, 0);
    assert_eq!(migrated.virtual_quote_reserves, 0);
}
