use naclac_client::*;
use dup_mut_client::{
    instructions::{
        build_init_vault_a, build_init_vault_b, build_init_vault_c, build_touch_pair_no_alias,
        build_touch_pair_with_alias, build_touch_triple_partial_alias, InitVaultAAccounts,
        InitVaultBAccounts, InitVaultCAccounts, TouchPairNoAliasAccounts,
        TouchPairWithAliasAccounts, TouchTriplePartialAliasAccounts,
    },
    get_vault_a_pda, get_vault_b_pda, get_vault_c_pda,
    types::PROGRAM_ID,
};

fn load_program(provider: &NaclacProvider) {
    let mut so_path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    so_path.pop(); // programs
    so_path.pop(); // dup-mut workspace root
    so_path.push("target/deploy/dup_mut.so");

    provider
        .add_program(&PROGRAM_ID, so_path.to_str().unwrap())
        .expect("Failed to load dup_mut program binary");
}

/// Asserts a transaction failed with exactly the given `Custom` error code
/// — not just "any error", the specific numeric `NaclacError` (framework
/// errors, 3000s) the failure actually produces. Mirrors
/// `tests/error-codes/programs/error_codes/tests/error_codes_test.rs`'s
/// helper of the same name and shape.
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

fn setup() -> (NaclacProvider, Address, Address) {
    let payer = load_node_wallet().expect("Failed to load local Solana keypair");
    let provider = NaclacProvider::new("litesvm", payer);
    load_program(&provider);

    let (vault_a_pda, _bump) = get_vault_a_pda(&PROGRAM_ID);
    let (vault_b_pda, _bump) = get_vault_b_pda(&PROGRAM_ID);

    build_init_vault_a(
        &provider,
        PROGRAM_ID,
        InitVaultAAccounts {
            payer: provider.payer.address(),
            vault_a: vault_a_pda,
            system_program: Address::default(),
        },
    )
    .send_and_confirm()
    .expect("init_vault_a should succeed");

    build_init_vault_b(
        &provider,
        PROGRAM_ID,
        InitVaultBAccounts {
            payer: provider.payer.address(),
            vault_b: vault_b_pda,
            system_program: Address::default(),
        },
    )
    .send_and_confirm()
    .expect("init_vault_b should succeed");

    (provider, vault_a_pda, vault_b_pda)
}

/// Same as `setup()` but also creates a third, independent vault — needed
/// only by the 3-mut-slot partial-aliasing tests below.
fn setup_with_c() -> (NaclacProvider, Address, Address, Address) {
    let (provider, vault_a_pda, vault_b_pda) = setup();
    let (vault_c_pda, _bump) = get_vault_c_pda(&PROGRAM_ID);

    build_init_vault_c(
        &provider,
        PROGRAM_ID,
        InitVaultCAccounts {
            payer: provider.payer.address(),
            vault_c: vault_c_pda,
            system_program: Address::default(),
        },
    )
    .send_and_confirm()
    .expect("init_vault_c should succeed");

    (provider, vault_a_pda, vault_b_pda, vault_c_pda)
}

/// Baseline: two genuinely distinct mutable accounts, no aliasing involved
/// at all — confirms the guard doesn't false-positive on ordinary usage.
#[test]
fn distinct_accounts_are_accepted() {
    let (provider, vault_a_pda, vault_b_pda) = setup();

    let result = build_touch_pair_no_alias(
        &provider,
        PROGRAM_ID,
        TouchPairNoAliasAccounts {
            a: vault_a_pda,
            b: vault_b_pda,
        },
    )
    .send_and_confirm();

    assert!(
        result.is_ok(),
        "two distinct mutable accounts should be accepted: {:?}",
        result.err()
    );
}

/// The actual regression case: the same account passed into two `mut`
/// slots with neither marked `unsafe(alias)` must be rejected by the
/// compile-time `MUT_MASK` / runtime `__duplicates` bitvec check in
/// accounts.rs's generated `load_and_validate` — not silently allowed to
/// double-mutate.
#[test]
fn duplicate_mutable_account_without_alias_is_rejected() {
    let (provider, vault_a_pda, _vault_b_pda) = setup();

    let result = build_touch_pair_no_alias(
        &provider,
        PROGRAM_ID,
        TouchPairNoAliasAccounts {
            a: vault_a_pda,
            b: vault_a_pda,
        },
    )
    .send_and_confirm();

    // The `__duplicates` bitvec / `MUT_MASK` intersection check in
    // `accounts.rs`'s generated `load_and_validate` runs before any field is
    // loaded and always reports index 0, regardless of which declared `mut`
    // fields actually collided (`.err(0)`, `accounts.rs`) ->
    // 3000 + 0*100 + ConstraintDuplicateMutableAccount(25) = 3025.
    assert_custom_code(result, 3025);
}

/// The escape hatch: both fields marked `unsafe(alias)` should let the
/// caller pass the same account for both slots.
#[test]
fn duplicate_mutable_account_with_alias_is_accepted() {
    let (provider, vault_a_pda, _vault_b_pda) = setup();

    let result = build_touch_pair_with_alias(
        &provider,
        PROGRAM_ID,
        TouchPairWithAliasAccounts {
            a: vault_a_pda,
            b: vault_a_pda,
        },
    )
    .send_and_confirm();

    assert!(
        result.is_ok(),
        "unsafe(alias) on both fields should allow passing the same account twice: {:?}",
        result.err()
    );
}

/// 3-mut-slot pairwise case, positive path: `a`/`b` (both marked
/// `unsafe(alias)`) are passed the same address, while `c` (not marked
/// `alias`) is a genuinely distinct account. Confirms `c` doesn't
/// false-positive just because *some* duplicate exists elsewhere in the
/// same instruction — the runtime `__duplicates` bitvec only sets bits for
/// the indices that actually collided (`a`, `b`), and `c`'s own bit is
/// never touched, so `MUT_MASK & __duplicates` stays empty.
#[test]
fn three_mut_slots_accepts_when_only_the_aliased_pair_collides() {
    let (provider, vault_a_pda, _vault_b_pda, vault_c_pda) = setup_with_c();

    let result = build_touch_triple_partial_alias(
        &provider,
        PROGRAM_ID,
        TouchTriplePartialAliasAccounts {
            a: vault_a_pda,
            b: vault_a_pda,
            c: vault_c_pda,
        },
    )
    .send_and_confirm();

    assert!(
        result.is_ok(),
        "an aliased a/b pair plus a genuinely distinct c should be accepted: {:?}",
        result.err()
    );
}

/// 3-mut-slot pairwise case, negative path: `c` (not marked `alias`) is
/// given the *same* address as `a` (which IS marked `alias`). `unsafe(alias)`
/// is not transitively "this address may duplicate anywhere" — every slot
/// that ends up sharing an address must opt out individually. The
/// address-collision scan sets both `a`'s and `c`'s bitvec positions
/// regardless of either field's own annotation, and `c`'s bit is still in
/// `MUT_MASK` (mut, no alias), so the guard must still fire.
#[test]
fn three_mut_slots_rejects_when_the_non_aliased_slot_collides() {
    let (provider, vault_a_pda, vault_b_pda, _vault_c_pda) = setup_with_c();

    let result = build_touch_triple_partial_alias(
        &provider,
        PROGRAM_ID,
        TouchTriplePartialAliasAccounts {
            a: vault_a_pda,
            b: vault_b_pda,
            c: vault_a_pda,
        },
    )
    .send_and_confirm();

    assert_custom_code(result, 3025);
}

/// Appending an extra account in the transaction's trailing (non-declared)
/// slot whose address collides with a declared `mut`, non-alias field must
/// be rejected too, even though that extra account was never named in the
/// `Accounts` struct at all.
#[test]
fn duplicate_mutable_account_via_remaining_accounts_is_rejected() {
    let (provider, vault_a_pda, vault_b_pda) = setup();

    let result = build_touch_pair_no_alias(
        &provider,
        PROGRAM_ID,
        TouchPairNoAliasAccounts {
            a: vault_a_pda,
            b: vault_b_pda,
        },
    )
    .remaining_accounts(vec![AccountMeta {
        address: vault_a_pda,
        is_signer: false,
        is_writable: false,
    }])
    .send_and_confirm();

    assert_custom_code(result, 3025);
}
