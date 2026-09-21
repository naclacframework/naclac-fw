use naclac_client::*;
use pump_fees_client::{
    instructions::{
        build_claim_social_fee_pda, build_claim_social_fee_pda_v2, build_create_social_fee_pda,
        build_initialize_fee_config, build_initialize_fee_program_global,
        build_extend_fee_config, build_reset_fee_sharing_config, build_reset_fee_sharing_config_v2,
        build_revoke_fee_sharing_authority, build_set_authority,
        build_set_claim_rate_limit, build_set_disable_flags, build_set_social_claim_authority,
        build_transfer_fee_sharing_authority, build_update_admin, build_update_fee_config,
        build_update_fee_shares, build_update_fee_shares_v2,
        build_update_stable_fee_config, build_upsert_fee_tiers, build_upsert_stable_fee_tiers,
        build_create_donation_fee_pda, build_crank_donation_fee_pda,
        ClaimSocialFeePdaAccounts, ClaimSocialFeePdaV2Accounts, CreateSocialFeePdaAccounts,
        CreateDonationFeePdaAccounts, CrankDonationFeePdaAccounts,
        ExtendFeeConfigAccounts, InitializeFeeConfigAccounts,
        InitializeFeeProgramGlobalAccounts, ResetFeeSharingConfigAccounts,
        ResetFeeSharingConfigV2Accounts, SetAuthorityAccounts, SetClaimRateLimitAccounts,
        SetDisableFlagsAccounts, SetSocialClaimAuthorityAccounts, UpdateAdminAccounts,
        UpdateFeeConfigAccounts, UpdateFeeSharesAccounts, UpdateFeeSharesV2Accounts,
        UpdateStableFeeConfigAccounts, UpsertFeeTiersAccounts,
        UpsertStableFeeTiersAccounts,
    },
    build_create_fee_sharing_config, get_coin_creator_vault_authority_pda, get_fee_config_pda,
    get_fee_program_global_pda, get_pump_creator_vault_pda, get_pump_fees_authority_pda,
    get_pump_global_pda, get_pool_pda, get_pool_authority_pda, get_donation_fee_pda_pda,
    get_epoch_tracker_pda, get_debouncer_pda, get_debouncer_ata_pda,
    get_sharing_config_pda, get_social_fee_pda_pda, fetch_fee_config,
    fetch_fee_program_global, fetch_sharing_config, fetch_social_fee_pda,
    fetch_donation_fee_pda,
    CreateFeeSharingConfigAccounts, Fees, FeeConfig, FeeProgramGlobal, FeeTier, Global, Shareholder,
    SocialFeePdaClaimed, CrankDonationFeePdaArgs, DonationFeePdaCranked,
    FEECONFIG_DISCRIMINATOR, FEEPROGRAMGLOBAL_DISCRIMINATOR, GLOBAL_DISCRIMINATOR, PROGRAM_ID,
    PUMP_PROGRAM_ID, DONATION_RELAY_PROGRAM_ID,
};
use pump_client::{
    fetch_bonding_curve, get_bonding_curve_pda, get_global_pda, get_metadata_pda, get_mint_authority_pda,
    instructions::{
        build_create, build_initialize, build_set_params, CreateAccounts, InitializeAccounts,
        SetParamsAccounts,
    },
    MPL_TOKEN_METADATA_PROGRAM_ID,
};
use pump_amm_client::PROGRAM_ID as PUMP_AMM_PROGRAM_ID;

/// Loads `pump`'s real compiled program alongside `pump_fees` in the same
/// provider â€” `create_fee_sharing_config` genuinely CPIs into it, so
/// exercising that path needs the real on-chain program, not a fixture.
fn load_pump_program(provider: &NaclacProvider) {
    let mut pump_root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    pump_root.pop(); // programs
    pump_root.pop(); // pump-fees workspace root
    pump_root.pop(); // pump-fun-program
    pump_root.push("pump-bonding-curve");
    let so_path = resolve_cargo_target_dir(&pump_root).join("deploy/pump.so");

    provider
        .add_program(&PUMP_PROGRAM_ID, so_path.to_str().unwrap())
        .expect("Failed to load pump.so");
}

/// `pump::create` genuinely CPIs into real Metaplex Token Metadata â€” anything
/// that loads `pump.so` and calls `create` needs this loaded too.
fn load_mpl_token_metadata_program(provider: &NaclacProvider) {
    let mut so_path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    so_path.pop(); // programs
    so_path.pop(); // pump-fees workspace root
    so_path.pop(); // pump-fun-program
    so_path.push("reference/pump-rust-client/artifacts/mpl_token_metadata.so");

    provider
        .add_program(&MPL_TOKEN_METADATA_PROGRAM_ID, so_path.to_str().unwrap())
        .expect("Failed to load mpl_token_metadata.so");
}

/// Loads `pump_amm`'s real compiled program alongside `pump_fees`/`pump` â€”
/// `reset_fee_sharing_config(_v2)`/`update_fee_shares(_v2)` genuinely CPI into
/// it (via `pump` in turn), so exercising those paths needs the real on-chain
/// program, not a fixture.
fn load_pump_amm_program(provider: &NaclacProvider) {
    let mut pump_amm_root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    pump_amm_root.pop(); // programs
    pump_amm_root.pop(); // pump-fees workspace root
    pump_amm_root.pop(); // pump-fun-program
    pump_amm_root.push("pump-amm");
    let so_path = resolve_cargo_target_dir(&pump_amm_root).join("deploy/pump_amm.so");

    provider
        .add_program(&PUMP_AMM_PROGRAM_ID, so_path.to_str().unwrap())
        .expect("Failed to load pump_amm.so");
}

/// Loads the test-only keypair `constants::ADMIN_PUBKEY` is pinned to
/// (`tests/wallets/test_admin.json`) â€” the real program's admin key's
/// private half isn't held by anyone, so `initialize_fee_config`'s golden
/// path can only be exercised against a keypair this project controls.
/// Loads `donation_relay`'s real compiled program (`examples/donation-relay`,
/// a sibling of `pump-fun-program` rather than nested under it â€” one more
/// `pop()` than `load_pump_amm_program`) â€” `crank_donation_fee_pda` genuinely
/// CPIs into it, so exercising that path needs the real on-chain program.
fn load_donation_relay_program(provider: &NaclacProvider) {
    let mut donation_relay_root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    donation_relay_root.pop(); // programs
    donation_relay_root.pop(); // pump-fees workspace root
    donation_relay_root.pop(); // pump-fun-program
    donation_relay_root.pop(); // examples
    donation_relay_root.push("donation-relay");
    let so_path = resolve_cargo_target_dir(&donation_relay_root).join("deploy/donation_relay.so");

    provider
        .add_program(&DONATION_RELAY_PROGRAM_ID, so_path.to_str().unwrap())
        .expect("Failed to load donation_relay.so");
}

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

fn setup() -> NaclacProvider {
    let payer = load_node_wallet().expect("Failed to load local Solana keypair");
    NaclacProvider::new("litesvm", payer).expect("Failed to construct NaclacProvider")
}

/// Mirrors `tests/token/programs/token/tests/token_test.rs`'s helper of the
/// same name and shape â€” asserts the exact numeric `NaclacError` code, not
/// just "any error".
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

fn discriminated_bytes<T: bytemuck::Pod>(disc: [u8; 8], value: &T) -> Vec<u8> {
    let mut out = Vec::with_capacity(8 + core::mem::size_of::<T>());
    out.extend_from_slice(&disc);
    out.extend_from_slice(bytemuck::bytes_of(value));
    out
}

/// Injects a `Global` account owned by the (not-yet-built) bonding curve
/// program at `get_pump_global_pda` â€” `initialize_fee_program_global` reads
/// `authority` off this foreign account, so it needs real backing data. No
/// way to create it via a real instruction since we don't own that program.
fn setup_pump_global(provider: &NaclacProvider, authority: Address) -> Address {
    let (pda, _bump) = get_pump_global_pda();
    // Matches what real `pump::initialize` actually produces: every field
    // zero-default except `authority`.
    let mut global = <Global as bytemuck::Zeroable>::zeroed();
    global.authority = authority;
    let data = discriminated_bytes(GLOBAL_DISCRIMINATOR, &global);
    provider
        .set_account(&pda, data, &PUMP_PROGRAM_ID, 10_000_000)
        .expect("inject pump_global fixture");
    pda
}

/// Injects a `FeeConfig` account directly, bypassing `initialize_fee_config`
/// entirely â€” most tests here just need a `FeeConfig` to already exist and
/// don't care about its creation path (`initialize_fee_config` itself is
/// covered separately, by `initialize_fee_config_creates_state` /
/// `initialize_fee_config_rejects_non_admin_signer`).
fn setup_fee_config(
    provider: &NaclacProvider,
    config_program_id: Address,
    admin: Address,
    flat_fees: Fees,
    fee_tiers: Vec<FeeTier>,
    stable_fee_tiers: Vec<FeeTier>,
) -> Address {
    let (pda, bump) = get_fee_config_pda(&PROGRAM_ID, &config_program_id);

    let zero_tier = FeeTier {
        market_cap_lamports_threshold: 0,
        fees: Fees {
            lp_fee_bps: 0,
            protocol_fee_bps: 0,
            creator_fee_bps: 0,
            ..Default::default()
        },
        ..Default::default()
    };
    let mut tiers_arr = [zero_tier; 50];
    for (i, t) in fee_tiers.iter().enumerate() {
        tiers_arr[i] = *t;
    }
    let mut stable_tiers_arr = [zero_tier; 50];
    for (i, t) in stable_fee_tiers.iter().enumerate() {
        stable_tiers_arr[i] = *t;
    }

    let cfg = FeeConfig {
        flat_fees,
        fee_tiers: tiers_arr,
        stable_fee_tiers: stable_tiers_arr,
        fee_tiers_len: fee_tiers.len() as u32,
        stable_fee_tiers_len: stable_fee_tiers.len() as u32,
        bump,
        admin,
        ..Default::default()
    };
    let data = discriminated_bytes(FEECONFIG_DISCRIMINATOR, &cfg);
    provider
        .set_account(&pda, data, &PROGRAM_ID, 10_000_000_000)
        .expect("inject fee_config fixture");
    pda
}

/// Returns `(fee_program_global, authority)` â€” the authority keypair is
/// handed back (not discarded) since several other instructions
/// (`set_claim_rate_limit`, `set_disable_flags`, ...) require signing as
/// this exact same admin authority afterward.
fn setup_fee_program_global(
    provider: &NaclacProvider,
    social_claim_authority: Address,
    claim_rate_limit: u64,
) -> (Address, Keypair) {
    let authority = Keypair::new();
    provider.airdrop(&authority.address(), 10_000_000_000).unwrap();
    setup_pump_global(provider, authority.address());

    let (fee_program_global_pda, _) = get_fee_program_global_pda(&PROGRAM_ID);
    let (pump_global_pda, _) = get_pump_global_pda();

    build_initialize_fee_program_global(
        provider,
        PROGRAM_ID,
        social_claim_authority,
        0,
        claim_rate_limit,
        InitializeFeeProgramGlobalAccounts {
            authority: authority.address(),
            pump_global: pump_global_pda,
            fee_program_global: fee_program_global_pda,
            system_program: SYSTEM_PROGRAM_ID,
        },
    )
    .signer(&authority)
    .log()
    .send_and_confirm()
    .expect("initialize_fee_program_global should succeed");

    (fee_program_global_pda, authority)
}

/// Real end-to-end flow: injects the foreign `pump_global` fixture, calls
/// the real `initialize_fee_program_global` instruction, and verifies the
/// resulting on-chain state. `pump_global.authority` is fully under our own
/// control (we wrote the fixture), so â€” unlike `initialize_fee_config` â€”
/// this instruction's golden path needs no hardcoded admin key.
#[test]
fn initialize_fee_program_global_creates_state() {
    let provider = setup();
    let social_claim_authority = Keypair::new().address();

    let (fee_program_global_pda, _authority) = setup_fee_program_global(&provider, social_claim_authority, 3600);

    let fetched = fetch_fee_program_global(&provider, &fee_program_global_pda)
        .expect("fee_program_global should be readable");
    assert_eq!(fetched.social_claim_authority, social_claim_authority);
    assert_eq!(fetched.claim_rate_limit, 3600);
    assert_eq!(fetched.disable_flags, 0);
}

#[test]
fn initialize_fee_config_rejects_non_admin_signer() {
    let provider = setup();
    let not_admin = Keypair::new();
    provider.airdrop(&not_admin.address(), 10_000_000_000).unwrap();

    let config_program_id = Keypair::new().address();
    let (fee_config_pda, fee_config_bump) = get_fee_config_pda(&PROGRAM_ID, &config_program_id);

    let result = build_initialize_fee_config(
        &provider,
        PROGRAM_ID,
        fee_config_bump,
        InitializeFeeConfigAccounts {
            admin: not_admin.address(),
            config_program_id,
            fee_config: fee_config_pda,
            system_program: SYSTEM_PROGRAM_ID,
        },
    )
    .signer(&not_admin)
    .log()
    .send_and_confirm();

    // `InitializeFeeConfig { admin, .. }` â€” `admin` is field index 0 ->
    // 3000 + 0*100 + ConstraintAddress(3) = 3003.
    assert_custom_code(result, 3003);
}

#[test]
fn initialize_fee_config_creates_state() {
    let provider = setup();
    let admin = load_test_admin_keypair();
    provider.airdrop(&admin.address(), 10_000_000_000).unwrap();

    let config_program_id = Keypair::new().address();
    let (fee_config_pda, fee_config_bump) = get_fee_config_pda(&PROGRAM_ID, &config_program_id);

    build_initialize_fee_config(
        &provider,
        PROGRAM_ID,
        fee_config_bump,
        InitializeFeeConfigAccounts {
            admin: admin.address(),
            config_program_id,
            fee_config: fee_config_pda,
            system_program: SYSTEM_PROGRAM_ID,
        },
    )
    .signer(&admin)
    .log()
    .send_and_confirm()
    .expect("initialize_fee_config should succeed for the real ADMIN_PUBKEY signer");

    let fee_config =
        fetch_fee_config(&provider, &fee_config_pda).expect("fee_config should be readable");
    assert_eq!(fee_config.bump, fee_config_bump);
    assert_eq!(fee_config.fee_tiers_len, 0);
    assert_eq!(fee_config.stable_fee_tiers_len, 0);
}


/// fees-06#14 (probe6, resolved earlier this project): `claim_social_fee_pda`
/// drains `social_fee_pda`'s entire native SOL balance down to rent-exemption
/// and transfers all of it to `recipient` â€” the same full-balance-sweep shape
/// as `sweep_buyback`. Exercises `create_social_fee_pda` -> top-up ->
/// `claim_social_fee_pda` end-to-end against the real compiled program.
#[test]
fn social_fee_pda_create_and_claim_v1_sweeps_native_sol() {
    let provider = setup();
    let social_claim_authority = Keypair::new();
    provider.airdrop(&social_claim_authority.address(), 1_000_000_000).unwrap();
    let (fee_program_global_pda, _authority) = setup_fee_program_global(&provider, social_claim_authority.address(), 0);

    let payer = Keypair::new();
    provider.airdrop(&payer.address(), 10_000_000_000).unwrap();

    let user_id = "alice".to_string();
    let platform = 0u8;
    let (social_fee_pda_addr, social_fee_pda_bump) =
        get_social_fee_pda_pda(&PROGRAM_ID, user_id.clone(), platform);

    build_create_social_fee_pda(
        &provider,
        PROGRAM_ID,
        user_id.clone(),
        platform,
        social_fee_pda_bump,
        CreateSocialFeePdaAccounts {
            payer: payer.address(),
            social_fee_pda: social_fee_pda_addr,
            system_program: SYSTEM_PROGRAM_ID,
            fee_program_global: fee_program_global_pda,
        },
    )
    .signer(&payer)
    .log()
    .send_and_confirm()
    .expect("create_social_fee_pda should succeed");

    // Top up well beyond rent-exemption so the claim has real balance to sweep.
    provider.set_account_lamports(&social_fee_pda_addr, 5_000_000_000).unwrap();

    let recipient = Keypair::new();
    provider.airdrop(&recipient.address(), 1_000_000).unwrap();
    let recipient_before = provider.get_balance(&recipient.address()).unwrap();

    build_claim_social_fee_pda(
        &provider,
        PROGRAM_ID,
        user_id,
        platform,
        ClaimSocialFeePdaAccounts {
            recipient: recipient.address(),
            social_fee_pda: social_fee_pda_addr,
            fee_program_global: fee_program_global_pda,
            social_claim_authority: social_claim_authority.address(),
        },
    )
    .signer(&social_claim_authority)
    .log()
    .send_and_confirm()
    .expect("claim_social_fee_pda should succeed");

    let recipient_after = provider.get_balance(&recipient.address()).unwrap();
    let amount_claimed = recipient_after - recipient_before;
    assert!(amount_claimed > 0, "recipient should have received swept lamports");

    let pda_state = fetch_social_fee_pda(&provider, &social_fee_pda_addr)
        .expect("social_fee_pda should be readable after claim");
    assert_eq!(pda_state.total_claimed, amount_claimed);
}

/// fees-06#9: the claim rate limit is inclusive (`>=`) and a soft no-op â€”
/// log and return, no revert, no state change â€” unlike `sweep_buyback`'s
/// strict + hard-revert rate limit.
///
/// Deliberately doesn't rely on any assumption about litesvm's default
/// `Clock::unix_timestamp` (there's no way to read or set it through
/// `NaclacProvider`, so any such assumption would be a guess): the first
/// claim uses `claim_rate_limit = 0` (unconditionally eligible, since
/// elapsed time is never negative), which both succeeds *and* establishes a
/// known `last_claimed`. `set_claim_rate_limit` then raises the limit far
/// above any realistic elapsed time between two back-to-back transactions
/// in the same test, so the second claim is deterministically rate-limited
/// regardless of what the clock's absolute value actually is.
#[test]
fn claim_social_fee_pda_v1_soft_no_ops_when_rate_limited() {
    let provider = setup();
    let social_claim_authority = Keypair::new();
    provider.airdrop(&social_claim_authority.address(), 1_000_000_000).unwrap();
    let (fee_program_global_pda, admin) = setup_fee_program_global(&provider, social_claim_authority.address(), 0);

    let payer = Keypair::new();
    provider.airdrop(&payer.address(), 10_000_000_000).unwrap();

    let user_id = "bob".to_string();
    let platform = 0u8;
    let (social_fee_pda_addr, social_fee_pda_bump) =
        get_social_fee_pda_pda(&PROGRAM_ID, user_id.clone(), platform);

    build_create_social_fee_pda(
        &provider,
        PROGRAM_ID,
        user_id.clone(),
        platform,
        social_fee_pda_bump,
        CreateSocialFeePdaAccounts {
            payer: payer.address(),
            social_fee_pda: social_fee_pda_addr,
            system_program: SYSTEM_PROGRAM_ID,
            fee_program_global: fee_program_global_pda,
        },
    )
    .signer(&payer)
    .log()
    .send_and_confirm()
    .expect("create_social_fee_pda should succeed");

    provider.set_account_lamports(&social_fee_pda_addr, 5_000_000_000).unwrap();

    let recipient = Keypair::new();
    provider.airdrop(&recipient.address(), 1_000_000).unwrap();

    // First claim: rate_limit = 0, always eligible. Establishes last_claimed.
    build_claim_social_fee_pda(
        &provider,
        PROGRAM_ID,
        user_id.clone(),
        platform,
        ClaimSocialFeePdaAccounts {
            recipient: recipient.address(),
            social_fee_pda: social_fee_pda_addr,
            fee_program_global: fee_program_global_pda,
            social_claim_authority: social_claim_authority.address(),
        },
    )
    .signer(&social_claim_authority)
    .log()
    .send_and_confirm()
    .expect("first claim (rate_limit=0) should succeed");

    // Raise the rate limit far above any realistic same-test elapsed time,
    // then fund the PDA again so a second, wrongly-succeeding sweep would
    // be observable.
    build_set_claim_rate_limit(
        &provider,
        PROGRAM_ID,
        1_000_000_000,
        SetClaimRateLimitAccounts {
            authority: admin.address(),
            fee_program_global: fee_program_global_pda,
        },
    )
    .signer(&admin)
    .log()
    .send_and_confirm()
    .expect("set_claim_rate_limit should succeed");

    provider.set_account_lamports(&social_fee_pda_addr, 5_000_000_000).unwrap();
    let recipient_before = provider.get_balance(&recipient.address()).unwrap();

    provider.expire_blockhash().unwrap();

    build_claim_social_fee_pda(
        &provider,
        PROGRAM_ID,
        user_id,
        platform,
        ClaimSocialFeePdaAccounts {
            recipient: recipient.address(),
            social_fee_pda: social_fee_pda_addr,
            fee_program_global: fee_program_global_pda,
            social_claim_authority: social_claim_authority.address(),
        },
    )
    .signer(&social_claim_authority)
    .log()
    .send_and_confirm()
    .expect("second claim should succeed even when rate-limited (soft no-op, not a revert)");

    let recipient_after = provider.get_balance(&recipient.address()).unwrap();
    assert_eq!(
        recipient_after, recipient_before,
        "rate-limited claim must not move any lamports"
    );
}

fn read_token_account_amount(provider: &NaclacProvider, address: &Address) -> u64 {
    let data = provider
        .get_account_data(address)
        .expect("token account should exist");
    u64::from_le_bytes(data[64..72].try_into().unwrap())
}

/// fees-06#14 follow-up (probe7, resolved earlier this project): `_v2` uses
/// the identical full-balance-sweep shape as v1, this time of SPL tokens â€”
/// `associated_social_fee_pda`'s entire balance moves to `associated_recipient`
/// (created idempotently by the instruction's own `init_if_needed`
/// constraint, so it's only *derived* here, never pre-created). Also
/// exercises decoding a real `#[event(alloc)]` return value: `Option<T>`-tagged
/// (`return_data[0] == 1` for `Some`), the inner bytes read via
/// `NaclacAllocEvent::decode` rather than a raw Borsh/bytemuck cast, since
/// `SocialFeePdaClaimed` has a `String` field and isn't `Pod`.
#[test]
fn claim_social_fee_pda_v2_sweeps_full_token_balance() {
    let provider = setup();
    let social_claim_authority = Keypair::new();
    provider.airdrop(&social_claim_authority.address(), 1_000_000_000).unwrap();
    let (fee_program_global_pda, _admin) =
        setup_fee_program_global(&provider, social_claim_authority.address(), 0);

    let payer = Keypair::new();
    provider.airdrop(&payer.address(), 10_000_000_000).unwrap();

    let user_id = "carol".to_string();
    let platform = 0u8;
    let (social_fee_pda_addr, social_fee_pda_bump) =
        get_social_fee_pda_pda(&PROGRAM_ID, user_id.clone(), platform);

    build_create_social_fee_pda(
        &provider,
        PROGRAM_ID,
        user_id.clone(),
        platform,
        social_fee_pda_bump,
        CreateSocialFeePdaAccounts {
            payer: payer.address(),
            social_fee_pda: social_fee_pda_addr,
            system_program: SYSTEM_PROGRAM_ID,
            fee_program_global: fee_program_global_pda,
        },
    )
    .signer(&payer)
    .log()
    .send_and_confirm()
    .expect("create_social_fee_pda should succeed");

    // Real SPL mint + a real ATA owned by social_fee_pda, funded with a
    // claimable balance â€” via naclac_client's genuine CPI-backed test
    // helpers, not hand-built account bytes.
    let mint_authority = Keypair::new();
    provider.airdrop(&mint_authority.address(), 10_000_000_000).unwrap();
    let mint = Keypair::new();
    create_mint(&provider, &mint, &mint_authority.address(), 6).expect("create_mint");

    let associated_social_fee_pda = create_ata(&provider, &mint.address(), &social_fee_pda_addr)
        .expect("create_ata for social_fee_pda");
    mint_to(
        &provider,
        &mint.address(),
        &associated_social_fee_pda,
        &mint_authority,
        750_000_000,
    )
    .expect("mint_to social_fee_pda's ata");

    let recipient = Keypair::new();
    provider.airdrop(&recipient.address(), 1_000_000).unwrap();
    // Not pre-created: `claim_social_fee_pda_v2`'s own `init_if_needed`
    // constraint creates it idempotently â€” only its deterministic address
    // is needed up front, derived the same way `create_ata` does internally.
    let (associated_recipient, _) = Address::find_program_address(
        &[
            recipient.address().as_ref(),
            TOKEN_PROGRAM_ID.as_ref(),
            mint.address().as_ref(),
        ],
        &ASSOCIATED_TOKEN_PROGRAM_ID,
    );

    let result = build_claim_social_fee_pda_v2(
        &provider,
        PROGRAM_ID,
        user_id,
        platform,
        ClaimSocialFeePdaV2Accounts {
            recipient: recipient.address(),
            social_fee_pda: social_fee_pda_addr,
            quote_mint: mint.address(),
            associated_social_fee_pda,
            associated_recipient,
            quote_token_program: TOKEN_PROGRAM_ID,
            associated_token_program: ASSOCIATED_TOKEN_PROGRAM_ID,
            fee_program_global: fee_program_global_pda,
            social_claim_authority: social_claim_authority.address(),
            system_program: SYSTEM_PROGRAM_ID,
        },
    )
    .signer(&social_claim_authority)
    .log()
    .send_and_confirm()
    .expect("claim_social_fee_pda_v2 should succeed");

    assert_eq!(
        read_token_account_amount(&provider, &associated_social_fee_pda),
        0,
        "social_fee_pda's ata should be fully drained"
    );
    assert_eq!(
        read_token_account_amount(&provider, &associated_recipient),
        750_000_000,
        "recipient's ata should hold the full swept balance"
    );

    assert_eq!(result.return_data[0], 1, "claim should succeed and return Some(event)");
    let event = <SocialFeePdaClaimed as NaclacAllocEvent>::decode(&result.return_data[1..])
        .expect("SocialFeePdaClaimed should decode");
    assert_eq!(event.amount_claimed, 750_000_000);
    assert_eq!(event.claimable_before, 750_000_000);
    assert_eq!(event.recipient_balance_before, 0);
    assert_eq!(event.recipient_balance_after, 750_000_000);

    let pda_state = fetch_social_fee_pda(&provider, &social_fee_pda_addr)
        .expect("social_fee_pda should be readable after claim");
    assert_eq!(pda_state.total_stable_claimed, 750_000_000);
}

/// The only instruction that CPIs into another naclac-generated program:
/// loads `pump`'s real compiled binary alongside `pump_fees`, drives it
/// through its own `initialize`/`create` to produce a genuine on-chain
/// `BondingCurve`, then calls `create_fee_sharing_config` and confirms the
/// real `migrate_bonding_curve_creator` CPI actually reassigned
/// `bonding_curve.creator` â€” not a fixture-injected approximation of either
/// program's state.
#[test]
fn create_fee_sharing_config_reassigns_creator_via_real_cpi() {
    let provider = setup();
    load_pump_program(&provider);
    load_mpl_token_metadata_program(&provider);

    let (pump_global_pda_manual, _) = get_global_pda(&PUMP_PROGRAM_ID);
    let pump_authority = Keypair::new();
    provider.airdrop(&pump_authority.address(), 10_000_000_000).unwrap();

    build_initialize(
        &provider,
        PUMP_PROGRAM_ID,
        InitializeAccounts {
            user: pump_authority.address(),
            global: pump_global_pda_manual,
            system_program: SYSTEM_PROGRAM_ID,
        },
    )
    .signer(&pump_authority)
    .log()
    .send_and_confirm()
    .expect("pump::initialize should succeed");

    let creator = Keypair::new();
    provider.airdrop(&creator.address(), 10_000_000_000).unwrap();
    let mint = Keypair::new();
    let (bonding_curve_pda, bonding_curve_bump) =
        get_bonding_curve_pda(&PUMP_PROGRAM_ID, &mint.address());
    let (mint_authority_pda, _) = get_mint_authority_pda(&PUMP_PROGRAM_ID);
    let (associated_bonding_curve_pda, _) = Address::find_program_address(
        &[bonding_curve_pda.as_ref(), TOKEN_PROGRAM_ID.as_ref(), mint.address().as_ref()],
        &ASSOCIATED_TOKEN_PROGRAM_ID,
    );
    let (metadata_pda, metadata_bump) = get_metadata_pda(&mint.address());

    build_create(
        &provider,
        PUMP_PROGRAM_ID,
        "Test Coin".to_string(),
        "TEST".to_string(),
        "https://example.com/metadata.json".to_string(),
        creator.address(),
        bonding_curve_bump,
        metadata_bump,
        CreateAccounts {
            user: creator.address(),
            mint_authority: mint_authority_pda,
            mint: mint.address(),
            bonding_curve: bonding_curve_pda,
            associated_bonding_curve: associated_bonding_curve_pda,
            global: pump_global_pda_manual,
            mpl_token_metadata: MPL_TOKEN_METADATA_PROGRAM_ID,
            metadata: metadata_pda,
            system_program: SYSTEM_PROGRAM_ID,
            token_program: TOKEN_PROGRAM_ID,
            associated_token_program: ASSOCIATED_TOKEN_PROGRAM_ID,
        },
    )
    .signer(&creator)
    .signer(&mint)
    .log()
    .send_and_confirm()
    .expect("pump::create should succeed");

    let (pump_global_pda, _) = get_pump_global_pda();
    assert_eq!(
        pump_global_pda, pump_global_pda_manual,
        "pump_fees's seeds::program-derived pump_global PDA must match pump's own"
    );

    let (sharing_config_pda, sharing_config_bump) =
        get_sharing_config_pda(&PROGRAM_ID, &mint.address());

    build_create_fee_sharing_config(
        &provider,
        PROGRAM_ID,
        bonding_curve_bump,
        sharing_config_bump,
        CreateFeeSharingConfigAccounts {
            payer: creator.address(),
            pump_global: pump_global_pda,
            mint: mint.address(),
            sharing_config: sharing_config_pda,
            system_program: SYSTEM_PROGRAM_ID,
            bonding_curve: bonding_curve_pda,
            pump_program: PUMP_PROGRAM_ID,
            pump_fees_authority: get_pump_fees_authority_pda(&PROGRAM_ID).0,
        },
    )
    .signer(&creator)
    .log()
    .send_and_confirm()
    .expect("create_fee_sharing_config should succeed");

    let sharing_config = fetch_sharing_config(&provider, &sharing_config_pda)
        .expect("sharing_config should be readable");
    assert_eq!(sharing_config.bump, sharing_config_bump);
    assert_eq!(sharing_config.version, 2);
    assert_eq!(sharing_config.status, 1, "status should be Active");
    assert_eq!(sharing_config.mint, mint.address());
    assert_eq!(sharing_config.admin, creator.address());
    assert_eq!(sharing_config.shareholders_len, 1);
    assert_eq!(sharing_config.shareholders[0].address, creator.address());
    assert_eq!(sharing_config.shareholders[0].share_bps, 10_000);

    let bonding_curve = fetch_bonding_curve(&provider, &bonding_curve_pda)
        .expect("bonding_curve should be readable after CPI");
    assert_eq!(
        bonding_curve.creator, sharing_config_pda,
        "migrate_bonding_curve_creator CPI should reassign creator to sharing_config"
    );
}

#[test]
fn set_authority_updates_authority() {
    let provider = setup();
    let social_claim_authority = Keypair::new().address();
    let (fee_program_global_pda, authority) =
        setup_fee_program_global(&provider, social_claim_authority, 0);

    let new_authority = Keypair::new().address();
    build_set_authority(
        &provider,
        PROGRAM_ID,
        new_authority,
        SetAuthorityAccounts {
            authority: authority.address(),
            fee_program_global: fee_program_global_pda,
        },
    )
    .signer(&authority)
    .log()
    .send_and_confirm()
    .expect("set_authority should succeed");

    let fetched = fetch_fee_program_global(&provider, &fee_program_global_pda)
        .expect("fee_program_global should be readable");
    assert_eq!(fetched.authority, new_authority);
}

/// Representative rejection test for the whole class of simple
/// `authority`-gated `FeeProgramGlobal` setters (`set_authority`,
/// `set_claim_rate_limit`, `set_disable_flags`, `set_social_claim_authority`)
/// â€” all four share the exact same `require!(authority == fee_program_global.authority, FeesError::InvalidAdmin)`
/// shape, so this is checked once rather than once per instruction.
#[test]
fn set_authority_rejects_non_authority_signer() {
    let provider = setup();
    let social_claim_authority = Keypair::new().address();
    let (fee_program_global_pda, _authority) =
        setup_fee_program_global(&provider, social_claim_authority, 0);

    let not_authority = Keypair::new();
    provider.airdrop(&not_authority.address(), 10_000_000_000).unwrap();

    let result = build_set_authority(
        &provider,
        PROGRAM_ID,
        Keypair::new().address(),
        SetAuthorityAccounts {
            authority: not_authority.address(),
            fee_program_global: fee_program_global_pda,
        },
    )
    .signer(&not_authority)
    .log()
    .send_and_confirm();

    // `FeesError::InvalidAdmin` is enum index 1 -> 6000 + 1 = 6001.
    assert_custom_code(result, 6001);
}

#[test]
fn update_admin_transfers_admin() {
    let provider = setup();
    let admin = Keypair::new();
    provider.airdrop(&admin.address(), 10_000_000_000).unwrap();
    let config_program_id = Keypair::new().address();
    let flat_fees = Fees {
        lp_fee_bps: 1,
        protocol_fee_bps: 1,
        creator_fee_bps: 1,
        ..Default::default()
    };
    let fee_config_pda =
        setup_fee_config(&provider, config_program_id, admin.address(), flat_fees, vec![], vec![]);

    let new_admin = Keypair::new().address();
    build_update_admin(
        &provider,
        PROGRAM_ID,
        UpdateAdminAccounts {
            admin: admin.address(),
            config_program_id,
            fee_config: fee_config_pda,
            new_admin,
        },
    )
    .signer(&admin)
    .log()
    .send_and_confirm()
    .expect("update_admin should succeed");

    let fetched =
        fetch_fee_config(&provider, &fee_config_pda).expect("fee_config should be readable");
    assert_eq!(fetched.admin, new_admin);
}

#[test]
fn set_claim_rate_limit_updates_value() {
    let provider = setup();
    let social_claim_authority = Keypair::new().address();
    let (fee_program_global_pda, authority) =
        setup_fee_program_global(&provider, social_claim_authority, 0);

    build_set_claim_rate_limit(
        &provider,
        PROGRAM_ID,
        7200,
        SetClaimRateLimitAccounts {
            authority: authority.address(),
            fee_program_global: fee_program_global_pda,
        },
    )
    .signer(&authority)
    .log()
    .send_and_confirm()
    .expect("set_claim_rate_limit should succeed");

    let fetched = fetch_fee_program_global(&provider, &fee_program_global_pda)
        .expect("fee_program_global should be readable");
    assert_eq!(fetched.claim_rate_limit, 7200);
}

#[test]
fn set_disable_flags_updates_value() {
    let provider = setup();
    let social_claim_authority = Keypair::new().address();
    let (fee_program_global_pda, authority) =
        setup_fee_program_global(&provider, social_claim_authority, 0);

    build_set_disable_flags(
        &provider,
        PROGRAM_ID,
        0x02,
        SetDisableFlagsAccounts {
            authority: authority.address(),
            fee_program_global: fee_program_global_pda,
        },
    )
    .signer(&authority)
    .log()
    .send_and_confirm()
    .expect("set_disable_flags should succeed");

    let fetched = fetch_fee_program_global(&provider, &fee_program_global_pda)
        .expect("fee_program_global should be readable");
    assert_eq!(fetched.disable_flags, 0x02);
}

#[test]
fn set_social_claim_authority_updates_value() {
    let provider = setup();
    let original_social_claim_authority = Keypair::new().address();
    let (fee_program_global_pda, authority) =
        setup_fee_program_global(&provider, original_social_claim_authority, 0);

    let new_social_claim_authority = Keypair::new().address();
    build_set_social_claim_authority(
        &provider,
        PROGRAM_ID,
        new_social_claim_authority,
        SetSocialClaimAuthorityAccounts {
            authority: authority.address(),
            fee_program_global: fee_program_global_pda,
        },
    )
    .signer(&authority)
    .log()
    .send_and_confirm()
    .expect("set_social_claim_authority should succeed");

    let fetched = fetch_fee_program_global(&provider, &fee_program_global_pda)
        .expect("fee_program_global should be readable");
    assert_eq!(fetched.social_claim_authority, new_social_claim_authority);
}

#[test]
fn update_fee_config_replaces_tiers_and_flat_fees() {
    let provider = setup();
    let admin = Keypair::new();
    provider.airdrop(&admin.address(), 10_000_000_000).unwrap();
    let config_program_id = Keypair::new().address();
    let initial_flat = Fees {
        lp_fee_bps: 1,
        protocol_fee_bps: 1,
        creator_fee_bps: 1,
        ..Default::default()
    };
    let fee_config_pda = setup_fee_config(
        &provider,
        config_program_id,
        admin.address(),
        initial_flat,
        vec![],
        vec![],
    );

    let new_flat = Fees {
        lp_fee_bps: 100,
        protocol_fee_bps: 50,
        creator_fee_bps: 25,
        ..Default::default()
    };
    let new_tier = FeeTier {
        market_cap_lamports_threshold: 0,
        fees: Fees {
            lp_fee_bps: 200,
            protocol_fee_bps: 80,
            creator_fee_bps: 40,
            ..Default::default()
        },
        ..Default::default()
    };

    build_update_fee_config(
        &provider,
        PROGRAM_ID,
        vec![new_tier],
        new_flat,
        UpdateFeeConfigAccounts {
            admin: admin.address(),
            config_program_id,
            fee_config: fee_config_pda,
        },
    )
    .signer(&admin)
    .log()
    .send_and_confirm()
    .expect("update_fee_config should succeed");

    let fetched =
        fetch_fee_config(&provider, &fee_config_pda).expect("fee_config should be readable");
    assert_eq!(fetched.flat_fees.lp_fee_bps, 100);
    assert_eq!(fetched.fee_tiers_len, 1);
    assert_eq!(fetched.fee_tiers[0].fees.lp_fee_bps, 200);
}

#[test]
fn update_stable_fee_config_replaces_stable_tiers() {
    let provider = setup();
    let admin = Keypair::new();
    provider.airdrop(&admin.address(), 10_000_000_000).unwrap();
    let config_program_id = Keypair::new().address();
    let flat_fees = Fees {
        lp_fee_bps: 1,
        protocol_fee_bps: 1,
        creator_fee_bps: 1,
        ..Default::default()
    };
    let fee_config_pda =
        setup_fee_config(&provider, config_program_id, admin.address(), flat_fees, vec![], vec![]);

    let new_stable_tier = FeeTier {
        market_cap_lamports_threshold: 0,
        fees: Fees {
            lp_fee_bps: 10,
            protocol_fee_bps: 5,
            creator_fee_bps: 2,
            ..Default::default()
        },
        ..Default::default()
    };

    build_update_stable_fee_config(
        &provider,
        PROGRAM_ID,
        vec![new_stable_tier],
        UpdateStableFeeConfigAccounts {
            admin: admin.address(),
            config_program_id,
            fee_config: fee_config_pda,
        },
    )
    .signer(&admin)
    .log()
    .send_and_confirm()
    .expect("update_stable_fee_config should succeed");

    let fetched =
        fetch_fee_config(&provider, &fee_config_pda).expect("fee_config should be readable");
    assert_eq!(fetched.stable_fee_tiers_len, 1);
    assert_eq!(fetched.stable_fee_tiers[0].fees.lp_fee_bps, 10);
}

/// Also exercises `upsert_fee_tiers`'s continuity check â€”
/// `FeesError::OffsetNotContinuous` (fees-05 cross-cutting #7) â€” since that's
/// real business logic specific to this instruction, not just an
/// admin-signer gate shared with everything else.
#[test]
fn upsert_fee_tiers_appends_and_rejects_noncontinuous_offset() {
    let provider = setup();
    let admin = Keypair::new();
    provider.airdrop(&admin.address(), 10_000_000_000).unwrap();
    let config_program_id = Keypair::new().address();
    let flat_fees = Fees {
        lp_fee_bps: 1,
        protocol_fee_bps: 1,
        creator_fee_bps: 1,
        ..Default::default()
    };
    let tier0 = FeeTier {
        market_cap_lamports_threshold: 0,
        fees: Fees {
            lp_fee_bps: 300,
            protocol_fee_bps: 100,
            creator_fee_bps: 50,
            ..Default::default()
        },
        ..Default::default()
    };
    let fee_config_pda = setup_fee_config(
        &provider,
        config_program_id,
        admin.address(),
        flat_fees,
        vec![tier0],
        vec![],
    );

    let tier1 = FeeTier {
        market_cap_lamports_threshold: 1_000_000_000,
        fees: Fees {
            lp_fee_bps: 200,
            protocol_fee_bps: 80,
            creator_fee_bps: 40,
            ..Default::default()
        },
        ..Default::default()
    };

    let accounts = || UpsertFeeTiersAccounts {
        admin: admin.address(),
        config_program_id,
        fee_config: fee_config_pda,
    };

    // offset == current_len (1) appends a new tier.
    build_upsert_fee_tiers(&provider, PROGRAM_ID, vec![tier1], 1, accounts())
        .signer(&admin)
        .log()
        .send_and_confirm()
        .expect("upsert_fee_tiers should succeed at offset == current_len");

    let fetched =
        fetch_fee_config(&provider, &fee_config_pda).expect("fee_config should be readable");
    assert_eq!(fetched.fee_tiers_len, 2);
    assert_eq!(fetched.fee_tiers[1].fees.lp_fee_bps, 200);

    // offset (5) > current_len (2) must be rejected.
    let result = build_upsert_fee_tiers(&provider, PROGRAM_ID, vec![tier1], 5, accounts())
        .signer(&admin)
        .log()
        .send_and_confirm();
    // `FeesError::OffsetNotContinuous` is enum index 4 -> 6000 + 4 = 6004.
    assert_custom_code(result, 6004);
}

#[test]
fn upsert_stable_fee_tiers_appends_tiers() {
    let provider = setup();
    let admin = Keypair::new();
    provider.airdrop(&admin.address(), 10_000_000_000).unwrap();
    let config_program_id = Keypair::new().address();
    let flat_fees = Fees {
        lp_fee_bps: 1,
        protocol_fee_bps: 1,
        creator_fee_bps: 1,
        ..Default::default()
    };
    let fee_config_pda =
        setup_fee_config(&provider, config_program_id, admin.address(), flat_fees, vec![], vec![]);

    let tier0 = FeeTier {
        market_cap_lamports_threshold: 0,
        fees: Fees {
            lp_fee_bps: 10,
            protocol_fee_bps: 5,
            creator_fee_bps: 2,
            ..Default::default()
        },
        ..Default::default()
    };

    build_upsert_stable_fee_tiers(
        &provider,
        PROGRAM_ID,
        vec![tier0],
        0,
        UpsertStableFeeTiersAccounts {
            admin: admin.address(),
            config_program_id,
            fee_config: fee_config_pda,
        },
    )
    .signer(&admin)
    .log()
    .send_and_confirm()
    .expect("upsert_stable_fee_tiers should succeed");

    let fetched =
        fetch_fee_config(&provider, &fee_config_pda).expect("fee_config should be readable");
    assert_eq!(fetched.stable_fee_tiers_len, 1);
    assert_eq!(fetched.stable_fee_tiers[0].fees.lp_fee_bps, 10);
}

/// Both fee-sharing-authority instructions are dead code in the real
/// program â€” confirmed via direct execution against the real bytecode
/// (`revoke_fee_sharing_authority.rs`/`transfer_fee_sharing_authority.rs`'s
/// own doc comments): they always revert with `DeprecatedInstruction`
/// before touching any accounts, regardless of signer or state.
#[test]
fn revoke_fee_sharing_authority_is_always_deprecated() {
    let provider = setup();

    let result = build_revoke_fee_sharing_authority(&provider, PROGRAM_ID)
        .log()
        .send_and_confirm();

    // `FeesError::DeprecatedInstruction` is enum index 23 -> 6000 + 23 = 6023.
    assert_custom_code(result, 6023);
}

#[test]
fn transfer_fee_sharing_authority_is_always_deprecated() {
    let provider = setup();

    let result = build_transfer_fee_sharing_authority(&provider, PROGRAM_ID)
        .log()
        .send_and_confirm();

    assert_custom_code(result, 6023);
}

/// `FeeConfig` is a fixed-size zero-copy `#[component]` in this
/// reimplementation (no growable `Vec<FeeTier>` the way the real, Borsh-based
/// program has), so `fee_config`'s real on-chain size never needs to reach
/// `FeeConfig::CURRENT_SIZE` in the first place â€” `initialize_fee_config`
/// already allocates the account at its full fixed size. This test still
/// exercises the real realloc path end-to-end (any signer may pay, no
/// admin/ownership check, per the real IDL) and confirms it's a safe no-op
/// growth rather than asserting a specific byte delta.
#[test]
fn extend_fee_config_reallocs_without_losing_data() {
    let provider = setup();
    let admin = Keypair::new();
    provider.airdrop(&admin.address(), 10_000_000_000).unwrap();
    let config_program_id = Keypair::new().address();
    let flat_fees = Fees {
        lp_fee_bps: 100,
        protocol_fee_bps: 50,
        creator_fee_bps: 25,
        ..Default::default()
    };
    let fee_config_pda =
        setup_fee_config(&provider, config_program_id, admin.address(), flat_fees, vec![], vec![]);

    let data_len_before = provider.get_account_data(&fee_config_pda).unwrap().len();

    let payer = Keypair::new();
    provider.airdrop(&payer.address(), 10_000_000_000).unwrap();

    build_extend_fee_config(
        &provider,
        PROGRAM_ID,
        ExtendFeeConfigAccounts {
            user: payer.address(),
            config_program_id,
            fee_config: fee_config_pda,
            system_program: SYSTEM_PROGRAM_ID,
        },
    )
    .signer(&payer)
    .log()
    .send_and_confirm()
    .expect("extend_fee_config should succeed for any signer");

    let data_len_after = provider.get_account_data(&fee_config_pda).unwrap().len();
    assert!(
        data_len_after >= data_len_before,
        "realloc must never shrink fee_config below its already-fixed size"
    );

    let fetched =
        fetch_fee_config(&provider, &fee_config_pda).expect("fee_config should still be readable");
    assert_eq!(fetched.admin, admin.address(), "existing data must survive the realloc");
    assert_eq!(fetched.flat_fees.lp_fee_bps, 100);
}

// --- reset_fee_sharing_config(_v2) / update_fee_shares(_v2) ---
//
// All four instructions genuinely CPI into the real `pump`/`pump_amm`
// programs (via `pump-client`/`pump-amm-client`), and the real
// `distribute_creator_fees_v2`/`transfer_creator_fees_to_pump_v2` targets only
// implement the wrapped-SOL path (`UnsupportedQuoteMint` otherwise) â€” so a
// genuine happy-path test needs the real WSOL mint address specifically, not
// an arbitrary fresh mint. `create_mint` can't produce an account at that
// fixed address (no keypair controls it), so its `Mint`/`TokenAccount` bytes
// are constructed directly and injected via `set_account`, mirroring
// `reference/fee-tier-probe/src/bin/probe11.rs`/`probe12.rs`'s approach.

fn spl_mint_account_data(mint_authority: Option<&Address>, decimals: u8) -> Vec<u8> {
    let mut data = vec![0u8; 82];
    if let Some(auth) = mint_authority {
        data[0..4].copy_from_slice(&1u32.to_le_bytes());
        data[4..36].copy_from_slice(&auth.to_bytes());
    }
    data[36..44].copy_from_slice(&0u64.to_le_bytes()); // supply
    data[44] = decimals;
    data[45] = 1; // is_initialized
    data
}

/// Standard SPL Token Account (165 bytes) layout, matching `probe11.rs`.
fn spl_token_account_data(mint: &Address, owner: &Address, amount: u64, is_native_reserve: Option<u64>) -> Vec<u8> {
    let mut data = vec![0u8; 165];
    data[0..32].copy_from_slice(&mint.to_bytes());
    data[32..64].copy_from_slice(&owner.to_bytes());
    data[64..72].copy_from_slice(&amount.to_le_bytes());
    data[108] = 1; // state = Initialized
    if let Some(reserve) = is_native_reserve {
        data[109..113].copy_from_slice(&1u32.to_le_bytes()); // COption::Some
        data[113..121].copy_from_slice(&reserve.to_le_bytes());
    }
    data
}

fn wsol_mint_address() -> Address {
    "So11111111111111111111111111111111111111112"
        .parse()
        .expect("WSOL mint address must parse")
}

/// SPL Token account rent-exempt reserve (165 bytes) â€” same value
/// `probe11.rs`/`probe12.rs` use.
const TOKEN_ACCOUNT_RENT_EXEMPT_RESERVE: u64 = 2_039_280;
/// Rent-exempt minimum for a 0-byte (lamport-only) account â€” empirically
/// observed via `probe11.rs`'s `AFTER: pump_creator_vault=890880` output.
const LAMPORT_VAULT_RENT_EXEMPT_MINIMUM: u64 = 890_880;

fn inject_wsol_mint(provider: &NaclacProvider) -> Address {
    let wsol = wsol_mint_address();
    provider
        .set_account(&wsol, spl_mint_account_data(None, 9), &TOKEN_PROGRAM_ID, 10_000_000)
        .expect("inject WSOL mint fixture");
    wsol
}

/// Real end-to-end setup shared by every `reset_fee_sharing_config(_v2)`/
/// `update_fee_shares(_v2)` test below: builds a genuine `BondingCurve` via
/// the real `pump` program, grants `pump_authority` control over
/// `global.admin_set_creator_authority` via `pump::set_params`, then migrates
/// the coin's creator to a fresh `sharing_config` via the real
/// `create_fee_sharing_config` â€” mirrors
/// `create_fee_sharing_config_reassigns_creator_via_real_cpi`'s setup.
///
/// `create_fee_sharing_config` always sets `shareholders[0].address = creator`
/// but `admin = payer`. When `decouple_admin_from_shareholder` is `true`,
/// `create_fee_sharing_config` is signed by `admin_set_creator_authority`
/// instead of `creator`, so `sharing_config.admin` ends up *different* from
/// its sole shareholder. Needed by any test that both signs as
/// `sharing_config.admin` *and* passes the sole current shareholder via
/// `remaining_accounts`: if the two were the same address (the `false`
/// case), naclac's accounts macro rejects the instruction outright as a
/// `ConstraintDuplicateMutableAccount` â€” a real aliasing-safety guard in
/// naclac's zero-copy account model that the real Anchor-based mainnet
/// program doesn't need (and so doesn't enforce the same way), not a bug in
/// the scenario itself. Tests that need `sharing_config.admin == creator`
/// specifically (to keep it distinguishable from `admin_set_creator_authority`)
/// must pass `false`.
/// Returns `(mint, creator, pump_authority, pump_global_pda, bonding_curve_pda,
/// bonding_curve_bump, sharing_config_pda, sharing_config_bump)`.
#[allow(clippy::type_complexity)]
fn setup_graduated_sharing_config(
    provider: &NaclacProvider,
    admin_set_creator_authority: &Keypair,
    decouple_admin_from_shareholder: bool,
) -> (Keypair, Keypair, Keypair, Address, Address, u8, Address, u8) {
    load_pump_program(provider);
    load_mpl_token_metadata_program(provider);
    load_pump_amm_program(provider);

    let (pump_global_pda, _) = get_global_pda(&PUMP_PROGRAM_ID);
    let pump_authority = Keypair::new();
    provider.airdrop(&pump_authority.address(), 10_000_000_000).unwrap();

    build_initialize(
        provider,
        PUMP_PROGRAM_ID,
        InitializeAccounts {
            user: pump_authority.address(),
            global: pump_global_pda,
            system_program: SYSTEM_PROGRAM_ID,
        },
    )
    .signer(&pump_authority)
    .send_and_confirm()
    .expect("pump::initialize should succeed");

    // `set_params` requires exactly 8 remaining accounts, each rent-exempt â€”
    // `remaining_accounts[0]` -> `Global.fee_recipient`,
    // `remaining_accounts[1..8]` -> `Global.fee_recipients` (confirmed real
    // mechanism, see `fees-07-donation-relay-progress.md`'s `probe21`). Their
    // exact addresses don't matter for this helper's callers.
    let fee_recipient_accounts: Vec<Address> = (0..8)
        .map(|_| {
            let address = Keypair::new().address();
            provider.airdrop(&address, 10_000_000_000).unwrap();
            address
        })
        .collect();

    build_set_params(
        provider,
        PUMP_PROGRAM_ID,
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
            admin_set_creator_authority: admin_set_creator_authority.address(),
            ..Default::default()
        },
        SetParamsAccounts {
            global: pump_global_pda,
            authority: pump_authority.address(),
        },
    )
    .signer(&pump_authority)
    .remaining_accounts(
        fee_recipient_accounts
            .iter()
            .map(|address| AccountMeta { address: *address, is_signer: false, is_writable: false })
            .collect(),
    )
    .send_and_confirm()
    .expect("pump::set_params should succeed");

    let creator = Keypair::new();
    provider.airdrop(&creator.address(), 10_000_000_000).unwrap();
    let mint = Keypair::new();
    let (bonding_curve_pda, bonding_curve_bump) =
        get_bonding_curve_pda(&PUMP_PROGRAM_ID, &mint.address());
    let (mint_authority_pda, _) = get_mint_authority_pda(&PUMP_PROGRAM_ID);
    let (associated_bonding_curve_pda, _) = Address::find_program_address(
        &[bonding_curve_pda.as_ref(), TOKEN_PROGRAM_ID.as_ref(), mint.address().as_ref()],
        &ASSOCIATED_TOKEN_PROGRAM_ID,
    );
    let (metadata_pda, metadata_bump) = get_metadata_pda(&mint.address());

    build_create(
        provider,
        PUMP_PROGRAM_ID,
        "Test Coin".to_string(),
        "TEST".to_string(),
        "https://example.com/metadata.json".to_string(),
        creator.address(),
        bonding_curve_bump,
        metadata_bump,
        CreateAccounts {
            user: creator.address(),
            mint_authority: mint_authority_pda,
            mint: mint.address(),
            bonding_curve: bonding_curve_pda,
            associated_bonding_curve: associated_bonding_curve_pda,
            global: pump_global_pda,
            mpl_token_metadata: MPL_TOKEN_METADATA_PROGRAM_ID,
            metadata: metadata_pda,
            system_program: SYSTEM_PROGRAM_ID,
            token_program: TOKEN_PROGRAM_ID,
            associated_token_program: ASSOCIATED_TOKEN_PROGRAM_ID,
        },
    )
    .signer(&creator)
    .signer(&mint)
    .send_and_confirm()
    .expect("pump::create should succeed");

    let (sharing_config_pda, sharing_config_bump) =
        get_sharing_config_pda(&PROGRAM_ID, &mint.address());

    let sharing_config_payer: &Keypair = if decouple_admin_from_shareholder {
        provider.airdrop(&admin_set_creator_authority.address(), 10_000_000_000).unwrap();
        admin_set_creator_authority
    } else {
        &creator
    };
    build_create_fee_sharing_config(
        provider,
        PROGRAM_ID,
        bonding_curve_bump,
        sharing_config_bump,
        CreateFeeSharingConfigAccounts {
            payer: sharing_config_payer.address(),
            pump_global: pump_global_pda,
            mint: mint.address(),
            sharing_config: sharing_config_pda,
            system_program: SYSTEM_PROGRAM_ID,
            bonding_curve: bonding_curve_pda,
            pump_program: PUMP_PROGRAM_ID,
            pump_fees_authority: get_pump_fees_authority_pda(&PROGRAM_ID).0,
        },
    )
    .signer(sharing_config_payer)
    .send_and_confirm()
    .expect("create_fee_sharing_config should succeed");

    (
        mint,
        creator,
        pump_authority,
        pump_global_pda,
        bonding_curve_pda,
        bonding_curve_bump,
        sharing_config_pda,
        sharing_config_bump,
    )
}

/// Funds the two real vaults `reset_fee_sharing_config(_v2)`/
/// `update_fee_shares(_v2)` sweep: the native-lamport `pump_creator_vault`
/// (under `pump`) and the WSOL `coin_creator_vault_ata` (under `pump_amm`).
/// Returns `(pump_creator_vault, pump_creator_vault_bump,
/// coin_creator_vault_authority, coin_creator_vault_authority_bump,
/// coin_creator_vault_ata)`.
fn fund_creator_fee_vaults(
    provider: &NaclacProvider,
    sharing_config_pda: &Address,
    wsol_mint: &Address,
    pump_creator_vault_lamports: u64,
    coin_creator_vault_wsol_amount: u64,
) -> (Address, u8, Address, u8, Address) {
    let (pump_creator_vault_pda, pump_creator_vault_bump) =
        get_pump_creator_vault_pda(sharing_config_pda);
    provider
        .set_account(&pump_creator_vault_pda, vec![], &SYSTEM_PROGRAM_ID, pump_creator_vault_lamports)
        .expect("inject pump_creator_vault fixture");

    let (coin_creator_vault_authority_pda, coin_creator_vault_authority_bump) =
        get_coin_creator_vault_authority_pda(sharing_config_pda);
    let (coin_creator_vault_ata, _) = Address::find_program_address(
        &[coin_creator_vault_authority_pda.as_ref(), TOKEN_PROGRAM_ID.as_ref(), wsol_mint.as_ref()],
        &ASSOCIATED_TOKEN_PROGRAM_ID,
    );
    provider
        .set_account(
            &coin_creator_vault_ata,
            spl_token_account_data(
                wsol_mint,
                &coin_creator_vault_authority_pda,
                coin_creator_vault_wsol_amount,
                Some(TOKEN_ACCOUNT_RENT_EXEMPT_RESERVE),
            ),
            &TOKEN_PROGRAM_ID,
            TOKEN_ACCOUNT_RENT_EXEMPT_RESERVE + coin_creator_vault_wsol_amount,
        )
        .expect("inject coin_creator_vault_ata fixture");

    (
        pump_creator_vault_pda,
        pump_creator_vault_bump,
        coin_creator_vault_authority_pda,
        coin_creator_vault_authority_bump,
        coin_creator_vault_ata,
    )
}

/// Full happy path against the real `pump`/`pump_amm` bytecode (WSOL,
/// hardcoded per `reset_fee_sharing_config`'s legacy account shape): sweeps
/// both vaults to the sole current shareholder, then overwrites
/// `sharing_config` with a single 100%-share `new_admin` entry at version 2.
#[test]
fn reset_fee_sharing_config_full_flow_sweeps_and_resets() {
    let provider = setup();
    let admin_set_creator_authority = Keypair::new();
    let (mint, creator, _pump_authority, pump_global_pda, bonding_curve_pda, bonding_curve_bump, sharing_config_pda, _sharing_config_bump) =
        setup_graduated_sharing_config(&provider, &admin_set_creator_authority, false);
    provider.airdrop(&admin_set_creator_authority.address(), 10_000_000_000).unwrap();

    let wsol_mint = inject_wsol_mint(&provider);
    let (pump_creator_vault_pda, pump_creator_vault_bump, coin_creator_vault_authority_pda, coin_creator_vault_authority_bump, coin_creator_vault_ata) =
        fund_creator_fee_vaults(&provider, &sharing_config_pda, &wsol_mint, 3_000_000_000, 2_000_000_000);

    let new_admin = Keypair::new().address();

    build_reset_fee_sharing_config(
        &provider,
        PROGRAM_ID,
        bonding_curve_bump,
        pump_creator_vault_bump,
        coin_creator_vault_authority_bump,
        ResetFeeSharingConfigAccounts {
            new_admin,
            authority: admin_set_creator_authority.address(),
            global: pump_global_pda,
            mint: mint.address(),
            sharing_config: sharing_config_pda,
            bonding_curve: bonding_curve_pda,
            pump_creator_vault: pump_creator_vault_pda,
            system_program: SYSTEM_PROGRAM_ID,
            pump_program: PUMP_PROGRAM_ID,
            pump_amm_program: PUMP_AMM_PROGRAM_ID,
            wsol_mint,
            token_program: TOKEN_PROGRAM_ID,
            associated_token_program: ASSOCIATED_TOKEN_PROGRAM_ID,
            coin_creator_vault_authority: coin_creator_vault_authority_pda,
            coin_creator_vault_ata,
            pump_fees_authority: get_pump_fees_authority_pda(&PROGRAM_ID).0,
        },
    )
    .signer(&admin_set_creator_authority)
    .remaining_accounts(vec![AccountMeta { address: creator.address(), is_signer: false, is_writable: true }])
    .log()
    .send_and_confirm()
    .expect("reset_fee_sharing_config should succeed");

    assert_eq!(
        provider.get_balance(&pump_creator_vault_pda).unwrap(),
        LAMPORT_VAULT_RENT_EXEMPT_MINIMUM,
        "pump_creator_vault should be swept down to its rent-exempt floor"
    );
    assert_eq!(
        read_token_account_amount(&provider, &coin_creator_vault_ata),
        0,
        "coin_creator_vault_ata should be fully swept"
    );

    let sharing_config = fetch_sharing_config(&provider, &sharing_config_pda)
        .expect("sharing_config should be readable");
    assert_eq!(sharing_config.admin, new_admin);
    assert_eq!(sharing_config.shareholders_len, 1);
    assert_eq!(sharing_config.shareholders[0].address, new_admin);
    assert_eq!(sharing_config.shareholders[0].share_bps, 10_000);
    assert_eq!(sharing_config.version, 2);
}

/// `pump::distribute_creator_fees` rejects any shareholder recipient that is
/// itself an executable program account (`UnableToDistributeCreatorFeesToExecutableRecipient`,
/// confirmed live against real deployed `pump.so` via `reference/fee-tier-probe/src/bin/probe73.rs`).
/// Only reachable through `pump_fees`'s real CPI (this check lives inside
/// `distribute_creator_fees`, whose `pump_fees_authority` signer can't be
/// faked from a standalone `pump`-only test), so this patches the real,
/// already-created `sharing_config`'s sole shareholder to `TOKEN_PROGRAM_ID`
/// (an already-loaded, genuinely executable account) after the real
/// `create_fee_sharing_config` CPI already ran, then drives the existing
/// `reset_fee_sharing_config` flow to reach it.
#[test]
fn reset_fee_sharing_config_rejects_executable_shareholder() {
    let provider = setup();
    let admin_set_creator_authority = Keypair::new();
    let (mint, _creator, _pump_authority, pump_global_pda, bonding_curve_pda, bonding_curve_bump, sharing_config_pda, _sharing_config_bump) =
        setup_graduated_sharing_config(&provider, &admin_set_creator_authority, false);
    provider.airdrop(&admin_set_creator_authority.address(), 10_000_000_000).unwrap();

    let mut sharing_config = fetch_sharing_config(&provider, &sharing_config_pda)
        .expect("sharing_config should be readable");
    sharing_config.shareholders[0].address = TOKEN_PROGRAM_ID;
    let sharing_config_lamports = provider.get_balance(&sharing_config_pda).unwrap();
    let data = discriminated_bytes(pump_fees_client::SHARINGCONFIG_DISCRIMINATOR, &sharing_config);
    provider
        .set_account(&sharing_config_pda, data, &PROGRAM_ID, sharing_config_lamports)
        .expect("patch sharing_config's sole shareholder to an executable account");

    let wsol_mint = inject_wsol_mint(&provider);
    let (pump_creator_vault_pda, pump_creator_vault_bump, coin_creator_vault_authority_pda, coin_creator_vault_authority_bump, coin_creator_vault_ata) =
        fund_creator_fee_vaults(&provider, &sharing_config_pda, &wsol_mint, 3_000_000_000, 2_000_000_000);

    let new_admin = Keypair::new().address();

    let result = build_reset_fee_sharing_config(
        &provider,
        PROGRAM_ID,
        bonding_curve_bump,
        pump_creator_vault_bump,
        coin_creator_vault_authority_bump,
        ResetFeeSharingConfigAccounts {
            new_admin,
            authority: admin_set_creator_authority.address(),
            global: pump_global_pda,
            mint: mint.address(),
            sharing_config: sharing_config_pda,
            bonding_curve: bonding_curve_pda,
            pump_creator_vault: pump_creator_vault_pda,
            system_program: SYSTEM_PROGRAM_ID,
            pump_program: PUMP_PROGRAM_ID,
            pump_amm_program: PUMP_AMM_PROGRAM_ID,
            wsol_mint,
            token_program: TOKEN_PROGRAM_ID,
            associated_token_program: ASSOCIATED_TOKEN_PROGRAM_ID,
            coin_creator_vault_authority: coin_creator_vault_authority_pda,
            coin_creator_vault_ata,
            pump_fees_authority: get_pump_fees_authority_pda(&PROGRAM_ID).0,
        },
    )
    .signer(&admin_set_creator_authority)
    .remaining_accounts(vec![AccountMeta { address: TOKEN_PROGRAM_ID, is_signer: false, is_writable: true }])
    .log()
    .send_and_confirm();

    // `PumpError::UnableToDistributeCreatorFeesToExecutableRecipient` is enum
    // index 31 in `pump`'s own `errors.rs` -> 6000 + 31 = 6031.
    assert_custom_code(result, 6031);
}

/// `authority` must equal `global.admin_set_creator_authority` â€” confirmed
/// empirically via `probe11.rs`. No vault funding needed: the check fires
/// before either nested CPI runs.
#[test]
fn reset_fee_sharing_config_rejects_wrong_authority() {
    let provider = setup();
    let admin_set_creator_authority = Keypair::new();
    let (mint, creator, _pump_authority, pump_global_pda, bonding_curve_pda, bonding_curve_bump, sharing_config_pda, _sharing_config_bump) =
        setup_graduated_sharing_config(&provider, &admin_set_creator_authority, false);

    let wsol_mint = inject_wsol_mint(&provider);
    let (pump_creator_vault_pda, pump_creator_vault_bump) =
        get_pump_creator_vault_pda(&sharing_config_pda);
    let (coin_creator_vault_authority_pda, coin_creator_vault_authority_bump) =
        get_coin_creator_vault_authority_pda(&sharing_config_pda);
    let (coin_creator_vault_ata, _) = Address::find_program_address(
        &[coin_creator_vault_authority_pda.as_ref(), TOKEN_PROGRAM_ID.as_ref(), wsol_mint.as_ref()],
        &ASSOCIATED_TOKEN_PROGRAM_ID,
    );

    // sharing_config.admin, deliberately not global.admin_set_creator_authority; already
    // funded by setup_graduated_sharing_config (a second airdrop here would replay the
    // same litesvm transfer transaction, failing with AlreadyProcessed).
    let not_authority = creator;

    let result = build_reset_fee_sharing_config(
        &provider,
        PROGRAM_ID,
        bonding_curve_bump,
        pump_creator_vault_bump,
        coin_creator_vault_authority_bump,
        ResetFeeSharingConfigAccounts {
            new_admin: Keypair::new().address(),
            authority: not_authority.address(),
            global: pump_global_pda,
            mint: mint.address(),
            sharing_config: sharing_config_pda,
            bonding_curve: bonding_curve_pda,
            pump_creator_vault: pump_creator_vault_pda,
            system_program: SYSTEM_PROGRAM_ID,
            pump_program: PUMP_PROGRAM_ID,
            pump_amm_program: PUMP_AMM_PROGRAM_ID,
            wsol_mint,
            token_program: TOKEN_PROGRAM_ID,
            associated_token_program: ASSOCIATED_TOKEN_PROGRAM_ID,
            coin_creator_vault_authority: coin_creator_vault_authority_pda,
            coin_creator_vault_ata,
            pump_fees_authority: get_pump_fees_authority_pda(&PROGRAM_ID).0,
        },
    )
    .signer(&not_authority)
    .log()
    .send_and_confirm();

    // `FeesError::NotAuthorized` is enum index 16 -> 6000 + 16 = 6016.
    assert_custom_code(result, 6016);
}

/// Full happy path for the generalized `_v2` variant (WSOL passed as a
/// generic `quote_mint`, not hardcoded) â€” same real-bytecode CPI chain as
/// `reset_fee_sharing_config_full_flow_sweeps_and_resets`. `pump_creator_vault_ata`
/// is a dead-but-declared account in this scoped WSOL-only pass (never read or
/// written), so an arbitrary unfunded address suffices for it.
#[test]
fn reset_fee_sharing_config_v2_full_flow_sweeps_and_resets() {
    let provider = setup();
    let admin_set_creator_authority = Keypair::new();
    let (mint, creator, _pump_authority, pump_global_pda, bonding_curve_pda, bonding_curve_bump, sharing_config_pda, _sharing_config_bump) =
        setup_graduated_sharing_config(&provider, &admin_set_creator_authority, false);
    provider.airdrop(&admin_set_creator_authority.address(), 10_000_000_000).unwrap();

    let wsol_mint = inject_wsol_mint(&provider);
    let (pump_creator_vault_pda, pump_creator_vault_bump, coin_creator_vault_authority_pda, coin_creator_vault_authority_bump, coin_creator_vault_ata) =
        fund_creator_fee_vaults(&provider, &sharing_config_pda, &wsol_mint, 3_000_000_000, 2_000_000_000);
    let pump_creator_vault_ata = Keypair::new().address();

    let new_admin = Keypair::new().address();

    build_reset_fee_sharing_config_v2(
        &provider,
        PROGRAM_ID,
        bonding_curve_bump,
        pump_creator_vault_bump,
        coin_creator_vault_authority_bump,
        ResetFeeSharingConfigV2Accounts {
            new_admin,
            authority: admin_set_creator_authority.address(),
            global: pump_global_pda,
            mint: mint.address(),
            sharing_config: sharing_config_pda,
            bonding_curve: bonding_curve_pda,
            pump_creator_vault: pump_creator_vault_pda,
            pump_creator_vault_ata,
            system_program: SYSTEM_PROGRAM_ID,
            pump_program: PUMP_PROGRAM_ID,
            pump_amm_program: PUMP_AMM_PROGRAM_ID,
            quote_mint: wsol_mint,
            token_program: TOKEN_PROGRAM_ID,
            associated_token_program: ASSOCIATED_TOKEN_PROGRAM_ID,
            coin_creator_vault_authority: coin_creator_vault_authority_pda,
            coin_creator_vault_ata,
            pump_fees_authority: get_pump_fees_authority_pda(&PROGRAM_ID).0,
        },
    )
    .signer(&admin_set_creator_authority)
    .remaining_accounts(vec![AccountMeta { address: creator.address(), is_signer: false, is_writable: true }])
    .log()
    .send_and_confirm()
    .expect("reset_fee_sharing_config_v2 should succeed");

    assert_eq!(
        provider.get_balance(&pump_creator_vault_pda).unwrap(),
        LAMPORT_VAULT_RENT_EXEMPT_MINIMUM
    );
    assert_eq!(read_token_account_amount(&provider, &coin_creator_vault_ata), 0);

    let sharing_config = fetch_sharing_config(&provider, &sharing_config_pda)
        .expect("sharing_config should be readable");
    assert_eq!(sharing_config.admin, new_admin);
    assert_eq!(sharing_config.shareholders_len, 1);
    assert_eq!(sharing_config.shareholders[0].address, new_admin);
    assert_eq!(sharing_config.version, 2);
}

/// Same real check as `reset_fee_sharing_config_rejects_executable_shareholder`,
/// but through the `_v2` payout path (`distribute_creator_fees_v2`, not v1) --
/// these are two separate functions in `pump`'s own source, so the check
/// needed its own dedicated test rather than assuming v1 coverage implies v2
/// is also exercised.
#[test]
fn reset_fee_sharing_config_v2_rejects_executable_shareholder() {
    let provider = setup();
    let admin_set_creator_authority = Keypair::new();
    let (mint, _creator, _pump_authority, pump_global_pda, bonding_curve_pda, bonding_curve_bump, sharing_config_pda, _sharing_config_bump) =
        setup_graduated_sharing_config(&provider, &admin_set_creator_authority, false);
    provider.airdrop(&admin_set_creator_authority.address(), 10_000_000_000).unwrap();

    let mut sharing_config = fetch_sharing_config(&provider, &sharing_config_pda)
        .expect("sharing_config should be readable");
    sharing_config.shareholders[0].address = TOKEN_PROGRAM_ID;
    let sharing_config_lamports = provider.get_balance(&sharing_config_pda).unwrap();
    let data = discriminated_bytes(pump_fees_client::SHARINGCONFIG_DISCRIMINATOR, &sharing_config);
    provider
        .set_account(&sharing_config_pda, data, &PROGRAM_ID, sharing_config_lamports)
        .expect("patch sharing_config's sole shareholder to an executable account");

    let wsol_mint = inject_wsol_mint(&provider);
    let (pump_creator_vault_pda, pump_creator_vault_bump, coin_creator_vault_authority_pda, coin_creator_vault_authority_bump, coin_creator_vault_ata) =
        fund_creator_fee_vaults(&provider, &sharing_config_pda, &wsol_mint, 3_000_000_000, 2_000_000_000);
    let pump_creator_vault_ata = Keypair::new().address();

    let new_admin = Keypair::new().address();

    let result = build_reset_fee_sharing_config_v2(
        &provider,
        PROGRAM_ID,
        bonding_curve_bump,
        pump_creator_vault_bump,
        coin_creator_vault_authority_bump,
        ResetFeeSharingConfigV2Accounts {
            new_admin,
            authority: admin_set_creator_authority.address(),
            global: pump_global_pda,
            mint: mint.address(),
            sharing_config: sharing_config_pda,
            bonding_curve: bonding_curve_pda,
            pump_creator_vault: pump_creator_vault_pda,
            pump_creator_vault_ata,
            system_program: SYSTEM_PROGRAM_ID,
            pump_program: PUMP_PROGRAM_ID,
            pump_amm_program: PUMP_AMM_PROGRAM_ID,
            quote_mint: wsol_mint,
            token_program: TOKEN_PROGRAM_ID,
            associated_token_program: ASSOCIATED_TOKEN_PROGRAM_ID,
            coin_creator_vault_authority: coin_creator_vault_authority_pda,
            coin_creator_vault_ata,
            pump_fees_authority: get_pump_fees_authority_pda(&PROGRAM_ID).0,
        },
    )
    .signer(&admin_set_creator_authority)
    .remaining_accounts(vec![AccountMeta { address: TOKEN_PROGRAM_ID, is_signer: false, is_writable: true }])
    .log()
    .send_and_confirm();

    // `PumpError::UnableToDistributeCreatorFeesToExecutableRecipient` is enum
    // index 31 in `pump`'s own `errors.rs` -> 6000 + 31 = 6031.
    assert_custom_code(result, 6031);
}

/// Full happy path for the legacy WSOL-hardcoded `update_fee_shares`: sets a
/// brand-new two-shareholder split and confirms the previous sole shareholder
/// (the coin creator) received the swept pending fees before the split
/// changed, matching `update_fee_shares_v2`'s confirmed mechanics.
#[test]
fn update_fee_shares_full_flow_updates_shareholders() {
    let provider = setup();
    let admin = Keypair::new(); // sharing_config.admin, deliberately not the sole shareholder (see setup_graduated_sharing_config's doc comment)
    let (mint, creator, _pump_authority, pump_global_pda, bonding_curve_pda, bonding_curve_bump, sharing_config_pda, _sharing_config_bump) =
        setup_graduated_sharing_config(&provider, &admin, true);

    let wsol_mint = inject_wsol_mint(&provider);
    let (pump_creator_vault_pda, pump_creator_vault_bump, coin_creator_vault_authority_pda, coin_creator_vault_authority_bump, coin_creator_vault_ata) =
        fund_creator_fee_vaults(&provider, &sharing_config_pda, &wsol_mint, 3_000_000_000, 2_000_000_000);
    let pump_creator_vault_ata = Keypair::new().address();

    let new_shareholder_a = Keypair::new().address();
    let new_shareholder_b = Keypair::new().address();

    build_update_fee_shares(
        &provider,
        PROGRAM_ID,
        bonding_curve_bump,
        pump_creator_vault_bump,
        coin_creator_vault_authority_bump,
        vec![
            Shareholder { address: new_shareholder_a, share_bps: 6_000, ..Default::default() },
            Shareholder { address: new_shareholder_b, share_bps: 4_000, ..Default::default() },
        ],
        UpdateFeeSharesAccounts {
            authority: admin.address(), // sharing_config.admin
            global: pump_global_pda,
            mint: mint.address(),
            sharing_config: sharing_config_pda,
            bonding_curve: bonding_curve_pda,
            pump_creator_vault: pump_creator_vault_pda,
            pump_creator_vault_ata,
            system_program: SYSTEM_PROGRAM_ID,
            pump_program: PUMP_PROGRAM_ID,
            pump_amm_program: PUMP_AMM_PROGRAM_ID,
            wsol_mint,
            token_program: TOKEN_PROGRAM_ID,
            associated_token_program: ASSOCIATED_TOKEN_PROGRAM_ID,
            coin_creator_vault_authority: coin_creator_vault_authority_pda,
            coin_creator_vault_ata,
            pump_fees_authority: get_pump_fees_authority_pda(&PROGRAM_ID).0,
        },
    )
    .signer(&admin)
    .remaining_accounts(vec![AccountMeta { address: creator.address(), is_signer: false, is_writable: true }])
    .log()
    .send_and_confirm()
    .expect("update_fee_shares should succeed");

    assert_eq!(provider.get_balance(&pump_creator_vault_pda).unwrap(), LAMPORT_VAULT_RENT_EXEMPT_MINIMUM);
    assert_eq!(read_token_account_amount(&provider, &coin_creator_vault_ata), 0);

    let sharing_config = fetch_sharing_config(&provider, &sharing_config_pda)
        .expect("sharing_config should be readable");
    assert_eq!(sharing_config.shareholders_len, 2);
    assert_eq!(sharing_config.shareholders[0].address, new_shareholder_a);
    assert_eq!(sharing_config.shareholders[0].share_bps, 6_000);
    assert_eq!(sharing_config.shareholders[1].address, new_shareholder_b);
    assert_eq!(sharing_config.shareholders[1].share_bps, 4_000);
    assert_eq!(sharing_config.admin_revoked, 1, "admin_revoked must be set so no further call can succeed");
    assert_eq!(sharing_config.admin, admin.address(), "update_fee_shares never changes .admin itself");
}

/// Full happy path for `update_fee_shares_v2`: sets a brand-new sole
/// shareholder and confirms the previous sole shareholder (the coin creator)
/// received the swept pending fees first, matching the confirmed mechanics
/// from `reference/fee-tier-probe/src/bin/probe12.rs`. `admin` (the real
/// `sharing_config.admin`, per `decouple_admin_from_shareholder = true`) is
/// deliberately a different keypair from `creator`.
#[test]
fn update_fee_shares_v2_full_flow_updates_shareholders_and_revokes_admin() {
    let provider = setup();
    let admin = Keypair::new(); // sharing_config.admin, deliberately not the sole shareholder
    let (mint, creator, _pump_authority, pump_global_pda, bonding_curve_pda, bonding_curve_bump, sharing_config_pda, _sharing_config_bump) =
        setup_graduated_sharing_config(&provider, &admin, true);

    let wsol_mint = inject_wsol_mint(&provider);
    let (pump_creator_vault_pda, pump_creator_vault_bump, coin_creator_vault_authority_pda, coin_creator_vault_authority_bump, coin_creator_vault_ata) =
        fund_creator_fee_vaults(&provider, &sharing_config_pda, &wsol_mint, 3_000_000_000, 2_000_000_000);
    let pump_creator_vault_ata = Keypair::new().address();

    let new_shareholder = Keypair::new().address();

    let accounts = || UpdateFeeSharesV2Accounts {
        authority: admin.address(), // sharing_config.admin
        global: pump_global_pda,
        mint: mint.address(),
        sharing_config: sharing_config_pda,
        bonding_curve: bonding_curve_pda,
        pump_creator_vault: pump_creator_vault_pda,
        pump_creator_vault_ata,
        system_program: SYSTEM_PROGRAM_ID,
        pump_program: PUMP_PROGRAM_ID,
        pump_amm_program: PUMP_AMM_PROGRAM_ID,
        quote_mint: wsol_mint,
        token_program: TOKEN_PROGRAM_ID,
        associated_token_program: ASSOCIATED_TOKEN_PROGRAM_ID,
        coin_creator_vault_authority: coin_creator_vault_authority_pda,
        coin_creator_vault_ata,
        pump_fees_authority: get_pump_fees_authority_pda(&PROGRAM_ID).0,
    };

    build_update_fee_shares_v2(
        &provider,
        PROGRAM_ID,
        bonding_curve_bump,
        pump_creator_vault_bump,
        coin_creator_vault_authority_bump,
        vec![Shareholder { address: new_shareholder, share_bps: 10_000, ..Default::default() }],
        accounts(),
    )
    .signer(&admin)
    .remaining_accounts(vec![AccountMeta { address: creator.address(), is_signer: false, is_writable: true }])
    .log()
    .send_and_confirm()
    .expect("update_fee_shares_v2 should succeed when signed by sharing_config.admin");

    assert_eq!(provider.get_balance(&pump_creator_vault_pda).unwrap(), LAMPORT_VAULT_RENT_EXEMPT_MINIMUM);
    assert_eq!(read_token_account_amount(&provider, &coin_creator_vault_ata), 0);

    let sharing_config = fetch_sharing_config(&provider, &sharing_config_pda)
        .expect("sharing_config should be readable");
    assert_eq!(sharing_config.shareholders_len, 1);
    assert_eq!(sharing_config.shareholders[0].address, new_shareholder);
    assert_eq!(sharing_config.shareholders[0].share_bps, 10_000);
    assert_eq!(sharing_config.admin_revoked, 1);
}

/// The probe12 regression, isolated: signing with `global.admin_set_creator_authority`
/// instead of `sharing_config.admin` must fail `NotAuthorized`, before any CPI
/// runs â€” no vault funding needed.
#[test]
fn update_fee_shares_v2_rejects_global_admin_set_creator_authority_signer() {
    let provider = setup();
    let admin_set_creator_authority = Keypair::new();
    // decouple_admin_from_shareholder = false: sharing_config.admin == creator here,
    // deliberately distinct from admin_set_creator_authority, so this test can prove
    // the latter is NOT sufficient authorization.
    let (mint, creator, _pump_authority, pump_global_pda, bonding_curve_pda, bonding_curve_bump, sharing_config_pda, _sharing_config_bump) =
        setup_graduated_sharing_config(&provider, &admin_set_creator_authority, false);
    provider.airdrop(&admin_set_creator_authority.address(), 10_000_000_000).unwrap();

    let wsol_mint = inject_wsol_mint(&provider);
    let (pump_creator_vault_pda, pump_creator_vault_bump) =
        get_pump_creator_vault_pda(&sharing_config_pda);
    let (coin_creator_vault_authority_pda, coin_creator_vault_authority_bump) =
        get_coin_creator_vault_authority_pda(&sharing_config_pda);
    let (coin_creator_vault_ata, _) = Address::find_program_address(
        &[coin_creator_vault_authority_pda.as_ref(), TOKEN_PROGRAM_ID.as_ref(), wsol_mint.as_ref()],
        &ASSOCIATED_TOKEN_PROGRAM_ID,
    );
    let pump_creator_vault_ata = Keypair::new().address();

    let result = build_update_fee_shares_v2(
        &provider,
        PROGRAM_ID,
        bonding_curve_bump,
        pump_creator_vault_bump,
        coin_creator_vault_authority_bump,
        vec![Shareholder { address: Keypair::new().address(), share_bps: 10_000, ..Default::default() }],
        UpdateFeeSharesV2Accounts {
            authority: admin_set_creator_authority.address(), // NOT sharing_config.admin
            global: pump_global_pda,
            mint: mint.address(),
            sharing_config: sharing_config_pda,
            bonding_curve: bonding_curve_pda,
            pump_creator_vault: pump_creator_vault_pda,
            pump_creator_vault_ata,
            system_program: SYSTEM_PROGRAM_ID,
            pump_program: PUMP_PROGRAM_ID,
            pump_amm_program: PUMP_AMM_PROGRAM_ID,
            quote_mint: wsol_mint,
            token_program: TOKEN_PROGRAM_ID,
            associated_token_program: ASSOCIATED_TOKEN_PROGRAM_ID,
            coin_creator_vault_authority: coin_creator_vault_authority_pda,
            coin_creator_vault_ata,
            pump_fees_authority: get_pump_fees_authority_pda(&PROGRAM_ID).0,
        },
    )
    .signer(&admin_set_creator_authority)
    .log()
    .send_and_confirm();

    let _ = creator;
    // `FeesError::NotAuthorized` is enum index 16 -> 6000 + 16 = 6016.
    assert_custom_code(result, 6016);
}

/// Calling `update_fee_shares_v2` a second time â€” even signed correctly â€”
/// must fail once `admin_revoked` is set, before any CPI runs.
#[test]
fn update_fee_shares_v2_rejects_second_call_after_admin_revoked() {
    let provider = setup();
    let admin = Keypair::new(); // sharing_config.admin, deliberately not the sole shareholder
    let (mint, creator, _pump_authority, pump_global_pda, bonding_curve_pda, bonding_curve_bump, sharing_config_pda, _sharing_config_bump) =
        setup_graduated_sharing_config(&provider, &admin, true);

    let wsol_mint = inject_wsol_mint(&provider);
    let (pump_creator_vault_pda, pump_creator_vault_bump, coin_creator_vault_authority_pda, coin_creator_vault_authority_bump, coin_creator_vault_ata) =
        fund_creator_fee_vaults(&provider, &sharing_config_pda, &wsol_mint, 3_000_000_000, 2_000_000_000);
    let pump_creator_vault_ata = Keypair::new().address();

    let accounts = || UpdateFeeSharesV2Accounts {
        authority: admin.address(),
        global: pump_global_pda,
        mint: mint.address(),
        sharing_config: sharing_config_pda,
        bonding_curve: bonding_curve_pda,
        pump_creator_vault: pump_creator_vault_pda,
        pump_creator_vault_ata,
        system_program: SYSTEM_PROGRAM_ID,
        pump_program: PUMP_PROGRAM_ID,
        pump_amm_program: PUMP_AMM_PROGRAM_ID,
        quote_mint: wsol_mint,
        token_program: TOKEN_PROGRAM_ID,
        associated_token_program: ASSOCIATED_TOKEN_PROGRAM_ID,
        coin_creator_vault_authority: coin_creator_vault_authority_pda,
        coin_creator_vault_ata,
        pump_fees_authority: get_pump_fees_authority_pda(&PROGRAM_ID).0,
    };

    build_update_fee_shares_v2(
        &provider,
        PROGRAM_ID,
        bonding_curve_bump,
        pump_creator_vault_bump,
        coin_creator_vault_authority_bump,
        vec![Shareholder { address: Keypair::new().address(), share_bps: 10_000, ..Default::default() }],
        accounts(),
    )
    .signer(&admin)
    .remaining_accounts(vec![AccountMeta { address: creator.address(), is_signer: false, is_writable: true }])
    .send_and_confirm()
    .expect("first update_fee_shares_v2 call should succeed");

    provider.expire_blockhash().unwrap();

    let result = build_update_fee_shares_v2(
        &provider,
        PROGRAM_ID,
        bonding_curve_bump,
        pump_creator_vault_bump,
        coin_creator_vault_authority_bump,
        vec![Shareholder { address: Keypair::new().address(), share_bps: 10_000, ..Default::default() }],
        accounts(),
    )
    .signer(&admin)
    .log()
    .send_and_confirm();

    // `FeesError::FeeSharesAlreadyUpdated` is enum index 24 -> 6000 + 24 = 6024.
    assert_custom_code(result, 6024);
}

/// Table of `validate_shareholders` rejections â€” all fire before either
/// nested CPI runs, so a single funded-vault-free setup covers every case.
#[test]
fn update_fee_shares_v2_rejects_invalid_shareholders() {
    let provider = setup();
    let admin_set_creator_authority = Keypair::new();
    let (mint, creator, _pump_authority, pump_global_pda, bonding_curve_pda, bonding_curve_bump, sharing_config_pda, _sharing_config_bump) =
        setup_graduated_sharing_config(&provider, &admin_set_creator_authority, false);

    let wsol_mint = inject_wsol_mint(&provider);
    let (pump_creator_vault_pda, pump_creator_vault_bump) =
        get_pump_creator_vault_pda(&sharing_config_pda);
    let (coin_creator_vault_authority_pda, coin_creator_vault_authority_bump) =
        get_coin_creator_vault_authority_pda(&sharing_config_pda);
    let (coin_creator_vault_ata, _) = Address::find_program_address(
        &[coin_creator_vault_authority_pda.as_ref(), TOKEN_PROGRAM_ID.as_ref(), wsol_mint.as_ref()],
        &ASSOCIATED_TOKEN_PROGRAM_ID,
    );
    let pump_creator_vault_ata = Keypair::new().address();

    let accounts = || UpdateFeeSharesV2Accounts {
        authority: creator.address(),
        global: pump_global_pda,
        mint: mint.address(),
        sharing_config: sharing_config_pda,
        bonding_curve: bonding_curve_pda,
        pump_creator_vault: pump_creator_vault_pda,
        pump_creator_vault_ata,
        system_program: SYSTEM_PROGRAM_ID,
        pump_program: PUMP_PROGRAM_ID,
        pump_amm_program: PUMP_AMM_PROGRAM_ID,
        quote_mint: wsol_mint,
        token_program: TOKEN_PROGRAM_ID,
        associated_token_program: ASSOCIATED_TOKEN_PROGRAM_ID,
        coin_creator_vault_authority: coin_creator_vault_authority_pda,
        coin_creator_vault_ata,
        pump_fees_authority: get_pump_fees_authority_pda(&PROGRAM_ID).0,
    };

    let run = |shareholders: Vec<Shareholder>| {
        provider.expire_blockhash().unwrap();
        build_update_fee_shares_v2(
            &provider,
            PROGRAM_ID,
            bonding_curve_bump,
            pump_creator_vault_bump,
            coin_creator_vault_authority_bump,
            shareholders,
            accounts(),
        )
        .signer(&creator)
        .send_and_confirm()
    };

    // Empty -> NoShareholders (enum index 10 -> 6010).
    assert_custom_code(run(vec![]), 6010);

    // Too many (31 > MAX_SHAREHOLDERS=30) -> TooManyShareholders (enum index 11 -> 6011).
    let too_many: Vec<Shareholder> = (0..31)
        .map(|_| Shareholder { address: Keypair::new().address(), share_bps: 1, ..Default::default() })
        .collect();
    assert_custom_code(run(too_many), 6011);

    // Duplicate address -> DuplicateShareholder (enum index 12 -> 6012).
    let dup_addr = Keypair::new().address();
    assert_custom_code(
        run(vec![
            Shareholder { address: dup_addr, share_bps: 5_000, ..Default::default() },
            Shareholder { address: dup_addr, share_bps: 5_000, ..Default::default() },
        ]),
        6012,
    );

    // Zero share_bps -> ZeroShareNotAllowed (enum index 17 -> 6017).
    assert_custom_code(
        run(vec![Shareholder { address: Keypair::new().address(), share_bps: 0, ..Default::default() }]),
        6017,
    );

    // Sum != 10_000 -> InvalidShareTotal (enum index 14 -> 6014).
    assert_custom_code(
        run(vec![Shareholder { address: Keypair::new().address(), share_bps: 9_999, ..Default::default() }]),
        6014,
    );
}

/// `remaining_accounts` must have exactly one entry per *current* shareholder
/// â€” omitting it must fail before either nested CPI runs.
#[test]
fn update_fee_shares_v2_rejects_remaining_accounts_mismatch() {
    let provider = setup();
    let admin_set_creator_authority = Keypair::new();
    let (mint, creator, _pump_authority, pump_global_pda, bonding_curve_pda, bonding_curve_bump, sharing_config_pda, _sharing_config_bump) =
        setup_graduated_sharing_config(&provider, &admin_set_creator_authority, false);

    let wsol_mint = inject_wsol_mint(&provider);
    let (pump_creator_vault_pda, pump_creator_vault_bump) =
        get_pump_creator_vault_pda(&sharing_config_pda);
    let (coin_creator_vault_authority_pda, coin_creator_vault_authority_bump) =
        get_coin_creator_vault_authority_pda(&sharing_config_pda);
    let (coin_creator_vault_ata, _) = Address::find_program_address(
        &[coin_creator_vault_authority_pda.as_ref(), TOKEN_PROGRAM_ID.as_ref(), wsol_mint.as_ref()],
        &ASSOCIATED_TOKEN_PROGRAM_ID,
    );
    let pump_creator_vault_ata = Keypair::new().address();

    let result = build_update_fee_shares_v2(
        &provider,
        PROGRAM_ID,
        bonding_curve_bump,
        pump_creator_vault_bump,
        coin_creator_vault_authority_bump,
        vec![Shareholder { address: Keypair::new().address(), share_bps: 10_000, ..Default::default() }],
        UpdateFeeSharesV2Accounts {
            authority: creator.address(),
            global: pump_global_pda,
            mint: mint.address(),
            sharing_config: sharing_config_pda,
            bonding_curve: bonding_curve_pda,
            pump_creator_vault: pump_creator_vault_pda,
            pump_creator_vault_ata,
            system_program: SYSTEM_PROGRAM_ID,
            pump_program: PUMP_PROGRAM_ID,
            pump_amm_program: PUMP_AMM_PROGRAM_ID,
            quote_mint: wsol_mint,
            token_program: TOKEN_PROGRAM_ID,
            associated_token_program: ASSOCIATED_TOKEN_PROGRAM_ID,
            coin_creator_vault_authority: coin_creator_vault_authority_pda,
            coin_creator_vault_ata,
            pump_fees_authority: get_pump_fees_authority_pda(&PROGRAM_ID).0,
        },
    )
    .signer(&creator)
    // No remaining_accounts, but sharing_config has 1 current shareholder.
    .log()
    .send_and_confirm();

    // `FeesError::NotEnoughRemainingAccounts` is enum index 13 -> 6000 + 13 = 6013.
    assert_custom_code(result, 6013);
}

// --- crank_donation_fee_pda ---
//
// `donation_relay` has no Rust integration test coverage of its own (only a
// `.test.ts`), and no other `pump_fees` test exercises this instruction â€”
// this is the only runtime proof (rather than just a clean build) that
// `DonatePubkeyConfigIdWithPayerV1Args`'s `message: ZcString` field
// round-trips correctly through the real CPI call, closing out the
// raw-pointer-cast soundness bug `naclac-macros`'s `#[instruction_args]`
// used to have for dynamic fields.

/// Bypasses `initialize_fee_program_global` â€” that instruction needs a
/// fixture `pump_global`, but `setup_graduated_sharing_config` already
/// performs a real `pump::initialize` at the same PDA, so injecting a
/// competing fixture there would corrupt that real account. Mirrors
/// `setup_fee_config`'s direct-injection approach instead.
fn setup_fee_program_global_fixture(provider: &NaclacProvider, authority: Address) -> Address {
    let (pda, bump) = get_fee_program_global_pda(&PROGRAM_ID);
    let global = FeeProgramGlobal {
        claim_rate_limit: 0,
        authority,
        social_claim_authority: Address::default(),
        bump,
        disable_flags: 0,
        reserved: [0u8; 256],
        ..Default::default()
    };
    let data = discriminated_bytes(FEEPROGRAMGLOBAL_DISCRIMINATOR, &global);
    provider
        .set_account(&pda, data, &PROGRAM_ID, 10_000_000)
        .expect("inject fee_program_global fixture");
    pda
}

#[test]
fn crank_donation_fee_pda_sweeps_wsol_and_lamport_excess_via_real_cpi() {
    let provider = setup();
    let admin_set_creator_authority = Keypair::new();
    let (mint, _creator, _pump_authority, _pump_global_pda, bonding_curve_pda, bonding_curve_bump, sharing_config_pda, _sharing_config_bump) =
        setup_graduated_sharing_config(&provider, &admin_set_creator_authority, false);
    load_donation_relay_program(&provider);

    let fee_program_global_pda =
        setup_fee_program_global_fixture(&provider, admin_set_creator_authority.address());

    let base_mint = mint.address();
    let (pool_authority_pda, pool_authority_bump) = get_pool_authority_pda(&base_mint);
    let (pool_pda, pool_bump) = get_pool_pda(&pool_authority_pda, &base_mint);
    let config_id = Keypair::new().address();
    let (donation_fee_pda_pda, donation_fee_pda_bump) =
        get_donation_fee_pda_pda(&PROGRAM_ID, &base_mint, &config_id);

    let payer = Keypair::new();
    provider.airdrop(&payer.address(), 10_000_000_000).unwrap();

    // Real end-to-end setup: `bonding_curve.complete` is false (no `migrate`
    // was ever called), so `pool`/`pool_authority` only need to be
    // seed-valid, matching the only real-world shape this instruction is
    // known to be used for (see `create_donation_fee_pda`'s module comment).
    build_create_donation_fee_pda(
        &provider,
        PROGRAM_ID,
        bonding_curve_bump,
        pool_authority_bump,
        pool_bump,
        donation_fee_pda_bump,
        CreateDonationFeePdaAccounts {
            payer: payer.address(),
            system_program: SYSTEM_PROGRAM_ID,
            fee_program_global: fee_program_global_pda,
            base_mint,
            bonding_curve: bonding_curve_pda,
            pool_authority: pool_authority_pda,
            pool: pool_pda,
            sharing_config: Keypair::new().address(),
            config_id,
            donation_fee_pda: donation_fee_pda_pda,
        },
    )
    .signer(&payer)
    .send_and_confirm()
    .expect("create_donation_fee_pda should succeed");

    let wsol_mint = inject_wsol_mint(&provider);
    let (donation_fee_pda_ata, donation_fee_pda_ata_bump) = Address::find_program_address(
        &[donation_fee_pda_pda.as_ref(), TOKEN_PROGRAM_ID.as_ref(), wsol_mint.as_ref()],
        &ASSOCIATED_TOKEN_PROGRAM_ID,
    );
    // Must be marked as a native (wrapped-SOL) SPL account â€” the crank calls
    // the real classic-Token `SyncNative` instruction on this account, which
    // rejects any account whose `is_native` `COption` isn't `Some`.
    let ata_wsol_amount = 42_000_000u64;
    provider
        .set_account(
            &donation_fee_pda_ata,
            spl_token_account_data(
                &wsol_mint,
                &donation_fee_pda_pda,
                ata_wsol_amount,
                Some(TOKEN_ACCOUNT_RENT_EXEMPT_RESERVE),
            ),
            &TOKEN_PROGRAM_ID,
            TOKEN_ACCOUNT_RENT_EXEMPT_RESERVE + ata_wsol_amount,
        )
        .expect("inject donation_fee_pda_ata fixture");

    // Real lamport excess above `donation_fee_pda`'s rent-exempt minimum â€”
    // the crank must sweep this too, on top of the ATA's WSOL balance.
    let lamport_excess = 7_591_840u64;
    let current_lamports = provider.get_account(&donation_fee_pda_pda).unwrap().lamports;
    provider
        .set_account_lamports(&donation_fee_pda_pda, current_lamports + lamport_excess)
        .expect("top up donation_fee_pda lamport excess");

    let (epoch_tracker_pda, epoch_tracker_bump) = get_epoch_tracker_pda(&config_id, &wsol_mint);
    let (debouncer_pda, debouncer_bump) = get_debouncer_pda(&config_id, &wsol_mint);
    let (debouncer_ata, debouncer_ata_bump) = get_debouncer_ata_pda(
        &ASSOCIATED_TOKEN_PROGRAM_ID,
        &debouncer_pda,
        &TOKEN_PROGRAM_ID,
        &wsol_mint,
    );

    let cranker = Keypair::new();
    provider.airdrop(&cranker.address(), 10_000_000_000).unwrap();

    let crank_timestamp = 1_700_000_000i64;
    provider
        .set_clock_unix_timestamp(crank_timestamp)
        .expect("set_clock_unix_timestamp should succeed on the litesvm backend");

    let result = build_crank_donation_fee_pda(
        &provider,
        PROGRAM_ID,
        CrankDonationFeePdaArgs {
            donation_fee_pda_ata_bump,
            epoch_tracker_bump,
            debouncer_bump,
            debouncer_ata_bump,
            ..Default::default()
        },
        CrankDonationFeePdaAccounts {
            payer: cranker.address(),
            system_program: SYSTEM_PROGRAM_ID,
            token_program: TOKEN_PROGRAM_ID,
            associated_token_program: ASSOCIATED_TOKEN_PROGRAM_ID,
            rent: RENT_SYSVAR_ID,
            fee_program_global: fee_program_global_pda,
            base_mint,
            config_id,
            donation_fee_pda: donation_fee_pda_pda,
            quote_mint: wsol_mint,
            donation_fee_pda_ata,
            donation_relay_program: DONATION_RELAY_PROGRAM_ID,
            mint_whitelist: Keypair::new().address(),
            epoch_tracker: epoch_tracker_pda,
            debouncer: debouncer_pda,
            debouncer_ata,
        },
    )
    .signer(&cranker)
    .log()
    .send_and_confirm()
    .expect("crank_donation_fee_pda should succeed");

    let expected_amount = ata_wsol_amount + lamport_excess;

    assert_eq!(result.return_data[0], 1, "crank should succeed and return Some(event)");
    let event: DonationFeePdaCranked = bytemuck::pod_read_unaligned(
        &result.return_data[1..1 + core::mem::size_of::<DonationFeePdaCranked>()],
    );
    assert_eq!(event.amount, expected_amount);
    assert_eq!(event.donation_fee_pda, donation_fee_pda_pda);
    assert_eq!(event.config_id, config_id);
    assert_eq!(event.base_mint, base_mint);
    assert_eq!(event.quote_mint, wsol_mint);
    // `setup_graduated_sharing_config` also creates a `sharing_config`, which
    // reassigns `bonding_curve.creator` away from the original creator wallet
    // to `sharing_config`'s own address (`migrate_bonding_curve_creator`) â€”
    // `create_donation_fee_pda` reads whatever `bonding_curve.creator` is at
    // that point, so it's `sharing_config_pda`, not the original creator.
    assert_eq!(event.creator, sharing_config_pda);
    assert_eq!(event.timestamp, crank_timestamp);

    let updated = fetch_donation_fee_pda(&provider, &donation_fee_pda_pda)
        .expect("donation_fee_pda should still be readable");
    assert_eq!(updated.total_donated, expected_amount);
    assert_eq!(updated.last_crank_ts, crank_timestamp);

    // Proves the base58 `message` (`base_mint + "," + creator`) genuinely
    // round-tripped through the real `donation_relay` CPI as a `ZcString` â€”
    // the pre-fix raw-pointer-cast bug would crash or corrupt memory before
    // ever reaching this real SPL transfer into the debouncer's WSOL ATA.
    let debouncer_ata_data = provider.get_account_data(&debouncer_ata).unwrap();
    let debouncer_ata_balance = u64::from_le_bytes(debouncer_ata_data[64..72].try_into().unwrap());
    assert_eq!(debouncer_ata_balance, expected_amount);
}
