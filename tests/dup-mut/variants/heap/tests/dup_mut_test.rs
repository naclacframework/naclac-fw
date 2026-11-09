use naclac_client::*;
use dup_mut_borsh_client::{
    instructions::{
        build_check_read_only_dup, build_init_vault_a, build_init_vault_b, build_init_vault_c,
        build_touch_boxed_vault, build_touch_pair_no_alias, build_touch_pair_with_alias,
        build_touch_triple_partial_alias, CheckReadOnlyDupAccounts, InitVaultAAccounts,
        InitVaultBAccounts, InitVaultCAccounts, TouchBoxedVaultAccounts,
        TouchPairNoAliasAccounts, TouchPairWithAliasAccounts, TouchTriplePartialAliasAccounts,
    },
    fetch_vault, get_vault_a_pda, get_vault_b_pda, get_vault_c_pda,
    types::PROGRAM_ID,
};

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
    let provider = NaclacProvider::new("litesvm", payer).expect("Failed to construct NaclacProvider");

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

    // The lowest collided `mut` slot is `a` (0):
    // 10000 + 0*100 + ConstraintDuplicateMutableAccount(25) = 10025.
    assert_custom_code(result, 10025);
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

    // `a` is `alias`, so `c` (slot 2) is the only collided `mut` slot:
    // 10000 + 2*100 + ConstraintDuplicateMutableAccount(25) = 10225.
    assert_custom_code(result, 10225);
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

    assert_custom_code(result, 10025);
}

/// `CheckReadOnlyDup` with every slot distinct, `program_repeat` holding the
/// executable System Program. Slots: `written` 0, `plain` 1, `repeat` 2,
/// `program_repeat` 3, `unmarked` 4.
fn distinct_read_only_accounts() -> CheckReadOnlyDupAccounts {
    CheckReadOnlyDupAccounts {
        written: Keypair::new().address(),
        plain: Keypair::new().address(),
        repeat: Keypair::new().address(),
        program_repeat: SYSTEM_PROGRAM_ID,
        unmarked: Keypair::new().address(),
    }
}

#[test]
fn distinct_read_only_accounts_are_accepted() {
    let (provider, _, _) = setup();
    build_check_read_only_dup(&provider, PROGRAM_ID, distinct_read_only_accounts())
        .send_and_confirm()
        .expect("distinct accounts must be accepted");
}

#[test]
fn a_dup_field_may_repeat_a_read_only_account() {
    let (provider, _, _) = setup();
    let mut accounts = distinct_read_only_accounts();
    accounts.repeat = accounts.plain;
    build_check_read_only_dup(&provider, PROGRAM_ID, accounts)
        .send_and_confirm()
        .expect("a `dup` field must accept a repeated read-only account");
}

#[test]
fn a_dup_field_accepts_a_repeat_that_carries_its_required_flags() {
    let (provider, _, _) = setup();
    let mut accounts = distinct_read_only_accounts();
    accounts.plain = SYSTEM_PROGRAM_ID;
    build_check_read_only_dup(&provider, PROGRAM_ID, accounts)
        .send_and_confirm()
        .expect("an executable repeat must satisfy `dup, executable`");
}

/// ConstraintExecutable(7) at `program_repeat`: 10000 + 3*100 + 7.
#[test]
fn a_dup_field_still_checks_the_flags_of_what_it_repeats() {
    let (provider, _, _) = setup();
    let mut accounts = distinct_read_only_accounts();
    accounts.program_repeat = accounts.plain;
    let result = build_check_read_only_dup(&provider, PROGRAM_ID, accounts).send_and_confirm();
    assert_custom_code(result, 10307);
}

/// ConstraintDuplicateMutableAccount(25) at `written`: 10000 + 0*100 + 25.
#[test]
fn a_dup_field_may_not_repeat_a_mutable_account() {
    let (provider, _, _) = setup();
    let mut accounts = distinct_read_only_accounts();
    accounts.repeat = accounts.written;
    let result = build_check_read_only_dup(&provider, PROGRAM_ID, accounts).send_and_confirm();
    assert_custom_code(result, 10025);
}

/// ConstraintDuplicateAccount(34) at `unmarked`: 10000 + 4*100 + 34.
#[test]
fn an_unmarked_field_may_not_repeat_a_read_only_account() {
    let (provider, _, _) = setup();
    let mut accounts = distinct_read_only_accounts();
    accounts.unmarked = accounts.plain;
    let result = build_check_read_only_dup(&provider, PROGRAM_ID, accounts).send_and_confirm();
    assert_custom_code(result, 10434);
}

#[test]
fn an_unmarked_repeat_is_rejected_after_a_dup_field_accepted_the_same_account() {
    let (provider, _, _) = setup();
    let mut accounts = distinct_read_only_accounts();
    accounts.repeat = accounts.plain;
    accounts.unmarked = accounts.plain;
    let result = build_check_read_only_dup(&provider, PROGRAM_ID, accounts).send_and_confirm();
    assert_custom_code(result, 10434);
}

#[test]
fn a_repeated_mutable_account_is_reported_as_mutable_on_an_unmarked_field() {
    let (provider, _, _) = setup();
    let mut accounts = distinct_read_only_accounts();
    accounts.unmarked = accounts.written;
    let result = build_check_read_only_dup(&provider, PROGRAM_ID, accounts).send_and_confirm();
    assert_custom_code(result, 10025);
}

#[test]
fn a_trailing_account_may_repeat_a_read_only_account() {
    let (provider, _, _) = setup();
    let accounts = distinct_read_only_accounts();
    let plain = accounts.plain;
    build_check_read_only_dup(&provider, PROGRAM_ID, accounts)
        .remaining_accounts(vec![AccountMeta {
            address: plain,
            is_signer: false,
            is_writable: false,
        }])
        .send_and_confirm()
        .expect("a trailing repeat of a read-only account must be accepted");
}

#[test]
fn a_trailing_repeat_of_a_mutable_account_is_rejected_after_an_inline_dup() {
    let (provider, _, _) = setup();
    let mut accounts = distinct_read_only_accounts();
    accounts.repeat = accounts.plain;
    let written = accounts.written;
    let result = build_check_read_only_dup(&provider, PROGRAM_ID, accounts)
        .remaining_accounts(vec![AccountMeta {
            address: written,
            is_signer: false,
            is_writable: false,
        }])
        .send_and_confirm();
    assert_custom_code(result, 10025);
}

/// `Box<Account<T>>` coverage: no test program in this repo boxes any
/// account field before this. Confirms the blanket impls added for
/// `Box<T>` in `core/src/wrappers/mod.rs` (`NaclacAccount`, `Owner`,
/// `ToAddress`, `AsRefByteSlice`, `ToAccountInfo`/`ToAccountInfos`,
/// `ToCpiHandle`/`ToCpiHandleMut`) make a boxed field load, validate, and
/// mutate identically to a bare `Account<T>` field — the same `vault_a` PDA
/// this file already exercises unboxed via `touch_pair_no_alias` is used
/// here through the boxed `target` field instead, so the only variable is
/// the box itself.
#[test]
fn boxed_account_field_loads_and_mutates_like_an_unboxed_one() {
    let (provider, vault_a_pda, _vault_b_pda) = setup();

    let before = fetch_vault(&provider, &vault_a_pda).expect("vault_a should exist after setup");

    let result = build_touch_boxed_vault(
        &provider,
        PROGRAM_ID,
        TouchBoxedVaultAccounts { target: vault_a_pda },
    )
    .send_and_confirm();

    assert!(
        result.is_ok(),
        "a Box<Account<Vault>> field should load and validate exactly like an unboxed one: {:?}",
        result.err()
    );

    let after = fetch_vault(&provider, &vault_a_pda).expect("vault_a should still exist");
    assert_eq!(
        after.balance,
        before.balance + 1,
        "the boxed field's DerefMut must genuinely persist the mutation back to the account"
    );
}
