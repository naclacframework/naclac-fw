use naclac_client::*;
use accounts_constraints_borsh_client::{
    fetch_ledger, fetch_note, fetch_vault,
    get_ledger_pda, get_note_pda, get_seeded_pda, get_vault_pda,
    instructions::{
        build_check_address, build_check_address_relational, build_check_executable,
        build_check_external_pda, build_check_owner, build_check_owner_relational,
        build_check_rent_exempt, build_close_vault, build_close_vault_self,
        build_init_if_needed_ledger, build_init_note, build_init_seeded, build_init_vault, build_related_vault,
        build_related_vault_custom_error, build_require_signer, build_touch_mut_vault,
        build_touch_seeded, CheckAddressAccounts, CheckAddressRelationalAccounts,
        CheckExecutableAccounts, CheckExternalPdaAccounts, CheckOwnerAccounts,
        CheckOwnerRelationalAccounts, CheckRentExemptAccounts,
        CloseVaultAccounts, CloseVaultSelfAccounts, InitIfNeededLedgerAccounts,
        InitNoteAccounts, InitSeededAccounts, InitVaultAccounts, RelatedVaultAccounts,
        RelatedVaultCustomErrorAccounts, RequireSignerAccounts, TouchMutVaultAccounts,
        TouchSeededAccounts,
    },
    types::PROGRAM_ID,
};

fn setup() -> NaclacProvider {
    let payer = load_node_wallet().expect("Failed to load local Solana keypair");
    NaclacProvider::new("litesvm", payer).expect("Failed to construct NaclacProvider")
}

/// Asserts a transaction failed with exactly the given `Custom` error code
/// â€” not just "any error", the specific numeric `NaclacError` (framework
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

/// `init` + `payer` + `space`: account created, discriminator written,
/// sized/funded via the explicit `space = 8 + size_of::<Vault>()` expression.
#[test]
fn init_creates_discriminator_and_rent_exempt_account() {
    let provider = setup();
    let (vault_pda, _bump) = get_vault_pda(&PROGRAM_ID);

    build_init_vault(
        &provider,
        PROGRAM_ID,
        InitVaultAccounts {
            payer: provider.payer.address(),
            vault: vault_pda,
            system_program: Address::default(),
        },
    )
    .send_and_confirm()
    .expect("init_vault should succeed");

    let vault = fetch_vault(&provider, &vault_pda).expect("vault should be readable after init");
    assert_eq!(vault.value, 0);
    assert_eq!(vault.admin, provider.payer.address());

    let account = provider.get_account(&vault_pda).expect("vault account should exist");
    assert!(account.lamports > 0, "init'd account must be funded (rent-exempt)");
}

/// `init_if_needed`: the first call creates the account and sets its value;
/// a second call with a *different* argument must not overwrite it â€” the
/// account-creation CPI is skipped, and (per the handler's own sentinel
/// guard) so is the value write.
#[test]
fn init_if_needed_second_call_does_not_overwrite() {
    let provider = setup();
    let (ledger_pda, _bump) = get_ledger_pda(&PROGRAM_ID);

    build_init_if_needed_ledger(
        &provider,
        PROGRAM_ID,
        42,
        InitIfNeededLedgerAccounts {
            payer: provider.payer.address(),
            ledger: ledger_pda,
            system_program: Address::default(),
        },
    )
    .send_and_confirm()
    .expect("first init_if_needed_ledger call should create the account");

    build_init_if_needed_ledger(
        &provider,
        PROGRAM_ID,
        99,
        InitIfNeededLedgerAccounts {
            payer: provider.payer.address(),
            ledger: ledger_pda,
            system_program: Address::default(),
        },
    )
    .send_and_confirm()
    .expect("second init_if_needed_ledger call must succeed as a no-op, not error");

    let ledger = fetch_ledger(&provider, &ledger_pda).expect("ledger should be readable");
    assert_eq!(
        ledger.value, 42,
        "second init_if_needed call must not re-initialize/overwrite existing data"
    );
}

/// `signer`: a required signature that isn't provided must be rejected;
/// providing it must succeed.
#[test]
fn signer_constraint_rejects_missing_signature_accepts_real_one() {
    let provider = setup();
    let (vault_pda, _bump) = get_vault_pda(&PROGRAM_ID);

    build_init_vault(
        &provider,
        PROGRAM_ID,
        InitVaultAccounts {
            payer: provider.payer.address(),
            vault: vault_pda,
            system_program: Address::default(),
        },
    )
    .send_and_confirm()
    .expect("init_vault should succeed");

    let authority = Keypair::new();

    let missing_sig_result = build_require_signer(
        &provider,
        PROGRAM_ID,
        RequireSignerAccounts {
            vault: vault_pda,
            authority: authority.address(),
        },
    )
    .send_and_confirm();
    assert!(
        missing_sig_result.is_err(),
        "require_signer must reject when `authority` doesn't actually sign the transaction"
    );

    build_require_signer(
        &provider,
        PROGRAM_ID,
        RequireSignerAccounts {
            vault: vault_pda,
            authority: authority.address(),
        },
    )
    .signer(&authority)
    .send_and_confirm()
    .expect("require_signer must succeed once `authority` actually signs");
}

/// `owner`: rejects an account whose owning program doesn't match. A fresh,
/// never-created keypair is implicitly owned by the System Program; an
/// already-initialized `Vault` PDA is owned by our own program instead.
#[test]
fn owner_constraint_rejects_wrong_owning_program() {
    let provider = setup();
    let (vault_pda, _bump) = get_vault_pda(&PROGRAM_ID);

    build_init_vault(
        &provider,
        PROGRAM_ID,
        InitVaultAccounts {
            payer: provider.payer.address(),
            vault: vault_pda,
            system_program: Address::default(),
        },
    )
    .send_and_confirm()
    .expect("init_vault should succeed");

    let system_owned = Keypair::new();
    build_check_owner(
        &provider,
        PROGRAM_ID,
        CheckOwnerAccounts {
            target: system_owned.address(),
        },
    )
    .send_and_confirm()
    .expect("a fresh, never-created account (implicitly System-owned) must be accepted");

    let wrong_owner_result = build_check_owner(
        &provider,
        PROGRAM_ID,
        CheckOwnerAccounts { target: vault_pda },
    )
    .send_and_confirm();
    // `CheckOwner { target }` â€” `target` is field index 0 ->
    // 3000 + 0*100 + ConstraintOwner(4) = 3004.
    assert_custom_code(wrong_owner_result, 3004);
}

/// `address`: rejects an account whose own key doesn't match the expected
/// constant â€” distinct from `owner` (checks the account's *own* key, not
/// its owning program).
#[test]
fn address_constraint_rejects_mismatched_key() {
    let provider = setup();

    build_check_address(
        &provider,
        PROGRAM_ID,
        CheckAddressAccounts {
            target: SYSTEM_PROGRAM_ID,
        },
    )
    .send_and_confirm()
    .expect("passing the real System Program account must be accepted");

    let mismatched_result = build_check_address(
        &provider,
        PROGRAM_ID,
        CheckAddressAccounts {
            target: provider.payer.address(),
        },
    )
    .send_and_confirm();
    // `CheckAddress { target }` â€” `target` is field index 0 ->
    // 3000 + 0*100 + ConstraintAddress(3) = 3003.
    assert_custom_code(mismatched_result, 3003);
}

/// `owner = <another field>` (relational form): `expected_owner` is
/// declared after `target` in the Accounts struct, proving the check
/// resolves it by index into the raw accounts slice rather than by
/// referencing an already-loaded local variable.
#[test]
fn owner_constraint_relational_rejects_wrong_owning_program() {
    let provider = setup();

    let system_owned = Keypair::new();
    build_check_owner_relational(
        &provider,
        PROGRAM_ID,
        CheckOwnerRelationalAccounts {
            target: system_owned.address(),
            expected_owner: SYSTEM_PROGRAM_ID,
        },
    )
    .send_and_confirm()
    .expect("a fresh, System-owned account must be accepted when expected_owner is the System Program");

    let (vault_pda, _bump) = get_vault_pda(&PROGRAM_ID);
    build_init_vault(
        &provider,
        PROGRAM_ID,
        InitVaultAccounts {
            payer: provider.payer.address(),
            vault: vault_pda,
            system_program: Address::default(),
        },
    )
    .send_and_confirm()
    .expect("init_vault should succeed");

    let wrong_owner_result = build_check_owner_relational(
        &provider,
        PROGRAM_ID,
        CheckOwnerRelationalAccounts {
            target: vault_pda,
            expected_owner: SYSTEM_PROGRAM_ID,
        },
    )
    .send_and_confirm();
    // `CheckOwnerRelational { target, expected_owner }` â€” `target` is field
    // index 0 -> 3000 + 0*100 + ConstraintOwner(4) = 3004.
    assert_custom_code(wrong_owner_result, 3004);
}

/// `address = <another field>` (relational form) â€” same ordering proof as
/// the `owner` relational test above.
#[test]
fn address_constraint_relational_rejects_mismatched_key() {
    let provider = setup();

    build_check_address_relational(
        &provider,
        PROGRAM_ID,
        CheckAddressRelationalAccounts {
            target: SYSTEM_PROGRAM_ID,
            expected_address: SYSTEM_PROGRAM_ID,
        },
    )
    .send_and_confirm()
    .expect("matching target/expected_address must be accepted");

    let mismatched_result = build_check_address_relational(
        &provider,
        PROGRAM_ID,
        CheckAddressRelationalAccounts {
            target: SYSTEM_PROGRAM_ID,
            expected_address: provider.payer.address(),
        },
    )
    .send_and_confirm();
    // `CheckAddressRelational { target, expected_address }` â€” `target` is
    // field index 0 -> 3000 + 0*100 + ConstraintAddress(3) = 3003.
    assert_custom_code(mismatched_result, 3003);
}

/// `mut`: the framework requires the account actually be marked writable on
/// the transaction. The generated SDK's typed builder always marks this
/// slot writable per the IDL, so the negative case is produced by taking
/// the built `InstructionBuilder` and flipping the raw `AccountMeta`
/// ourselves â€” proving the runtime check, not just the SDK's own honesty.
#[test]
fn mut_constraint_rejects_non_writable_account_meta() {
    let provider = setup();
    let (vault_pda, _bump) = get_vault_pda(&PROGRAM_ID);

    build_init_vault(
        &provider,
        PROGRAM_ID,
        InitVaultAccounts {
            payer: provider.payer.address(),
            vault: vault_pda,
            system_program: Address::default(),
        },
    )
    .send_and_confirm()
    .expect("init_vault should succeed");

    build_touch_mut_vault(
        &provider,
        PROGRAM_ID,
        TouchMutVaultAccounts { vault: vault_pda },
    )
    .send_and_confirm()
    .expect("touch_mut_vault with a correctly-writable account must succeed");

    let mut builder = build_touch_mut_vault(
        &provider,
        PROGRAM_ID,
        TouchMutVaultAccounts { vault: vault_pda },
    );
    let vault_idx = builder
        .account_names
        .iter()
        .position(|name| *name == "vault")
        .expect("vault account slot must be present in the built instruction");
    builder.accounts[vault_idx].is_writable = false;

    let result = builder.send_and_confirm();
    // `TouchMutVault { vault }` â€” `vault` is field index 0 ->
    // 3000 + 0*100 + ConstraintMut(1) = 3001.
    assert_custom_code(result, 3001);
}

/// The `has_one`-equivalent relation mechanism (`admin = authority`):
/// rejects when `vault.admin` doesn't match the `authority` account passed
/// in, accepts when it does.
#[test]
fn relation_constraint_rejects_mismatched_admin() {
    let provider = setup();
    let (vault_pda, _bump) = get_vault_pda(&PROGRAM_ID);

    build_init_vault(
        &provider,
        PROGRAM_ID,
        InitVaultAccounts {
            payer: provider.payer.address(),
            vault: vault_pda,
            system_program: Address::default(),
        },
    )
    .send_and_confirm()
    .expect("init_vault should succeed");
    // `init_vault` sets `vault.admin = payer.address()`.

    let stranger = Keypair::new();
    let mismatched_result = build_related_vault(
        &provider,
        PROGRAM_ID,
        RelatedVaultAccounts {
            vault: vault_pda,
            authority: stranger.address(),
        },
    )
    .signer(&stranger)
    .send_and_confirm();
    // `RelatedVault { vault, authority }` â€” the relation check (`admin =
    // authority`) is attached to `vault`, field index 0, and (no
    // `custom_error` given) emits `NaclacError::Unauthorized` ->
    // 3000 + 0*100 + Unauthorized(21) = 3021.
    assert_custom_code(mismatched_result, 3021);

    build_related_vault(
        &provider,
        PROGRAM_ID,
        RelatedVaultAccounts {
            vault: vault_pda,
            authority: provider.payer.address(),
        },
    )
    .send_and_confirm()
    .expect("related_vault must succeed when `authority` matches `vault.admin`");
}

/// `seeds` + explicit `bump = <expr>` on an existing account: the correct
/// stored bump is accepted, a deliberately wrong one is rejected
/// (`NaclacError::ConstraintSeeds`) â€” the on-chain
/// `find_program_address`-banned hash-and-compare path.
#[test]
fn seeds_constraint_rejects_wrong_explicit_bump() {
    let provider = setup();
    let (seeded_pda, seeded_bump) = get_seeded_pda(&PROGRAM_ID);

    build_init_seeded(
        &provider,
        PROGRAM_ID,
        InitSeededAccounts {
            payer: provider.payer.address(),
            seeded: seeded_pda,
            system_program: Address::default(),
        },
    )
    .send_and_confirm()
    .expect("init_seeded should succeed");

    build_touch_seeded(
        &provider,
        PROGRAM_ID,
        seeded_bump,
        TouchSeededAccounts { seeded: seeded_pda },
    )
    .send_and_confirm()
    .expect("touch_seeded with the correct bump should succeed");

    let wrong_bump = seeded_bump.wrapping_sub(1);
    let wrong_result = build_touch_seeded(
        &provider,
        PROGRAM_ID,
        wrong_bump,
        TouchSeededAccounts { seeded: seeded_pda },
    )
    .send_and_confirm();
    // `TouchSeeded { seeded }` â€” `seeded` is field index 0 ->
    // 3000 + 0*100 + ConstraintSeeds(6) = 3006.
    assert_custom_code(wrong_result, 3006);
}

/// `close`: drains the target's lamports to `destination` and zeroes its
/// data; the account can't be loaded as a `Vault` again afterward.
#[test]
fn close_drains_lamports_and_account_cannot_be_reused() {
    let provider = setup();
    let (vault_pda, _bump) = get_vault_pda(&PROGRAM_ID);

    build_init_vault(
        &provider,
        PROGRAM_ID,
        InitVaultAccounts {
            payer: provider.payer.address(),
            vault: vault_pda,
            system_program: Address::default(),
        },
    )
    .send_and_confirm()
    .expect("init_vault should succeed");

    let payer_lamports_before = provider
        .get_account(&provider.payer.address())
        .expect("payer account should exist")
        .lamports;

    build_close_vault(
        &provider,
        PROGRAM_ID,
        CloseVaultAccounts {
            payer: provider.payer.address(),
            vault: vault_pda,
        },
    )
    .send_and_confirm()
    .expect("close_vault should succeed");

    let payer_lamports_after = provider
        .get_account(&provider.payer.address())
        .expect("payer account should exist")
        .lamports;
    assert!(
        payer_lamports_after > payer_lamports_before,
        "closing the vault must transfer its lamports to the payer"
    );

    let reuse_result = build_touch_mut_vault(
        &provider,
        PROGRAM_ID,
        TouchMutVaultAccounts { vault: vault_pda },
    )
    .send_and_confirm();
    // `close = payer` reassigns `vault` to the System Program and zeroes its
    // data (`close_account.rs`). Reloading it via `TouchMutVault { vault }`
    // (field index 0) hits `security.rs`'s default owner check (metadata
    // check, runs before `Account::try_from`'s discriminator check) first:
    // owner is now the System Program, not our program -> ConstraintOwner(4).
    // 3000 + 0*100 + 4 = 3004.
    assert_custom_code(reuse_result, 3004);
}

/// `executable`: rejects an account whose `executable` flag isn't set.
/// Meaningful specifically on a plain `AccountInfo` field (not `Program<T>`,
/// which already enforces executable-ness internally on its own) â€” the real
/// System Program account (genuinely executable) must be accepted, and the
/// payer's own signer account (not executable) must be rejected.
#[test]
fn executable_constraint_rejects_non_executable_account() {
    let provider = setup();

    build_check_executable(
        &provider,
        PROGRAM_ID,
        CheckExecutableAccounts {
            target: SYSTEM_PROGRAM_ID,
        },
    )
    .send_and_confirm()
    .expect("the real System Program account (executable) must be accepted");

    let non_executable_result = build_check_executable(
        &provider,
        PROGRAM_ID,
        CheckExecutableAccounts {
            target: provider.payer.address(),
        },
    )
    .send_and_confirm();
    // `CheckExecutable { target }` â€” `target` is field index 0 ->
    // 3000 + 0*100 + ConstraintExecutable(7) = 3007.
    assert_custom_code(non_executable_result, 3007);
}

/// `rent_exempt`: rejects an account whose lamport balance is below the
/// rent-exempt minimum for its (0-byte) data length. The default payer is
/// funded with a huge SOL balance by `NaclacProvider::new_litesvm`, so it
/// trivially qualifies as rent-exempt for a 0-byte account (~890880
/// lamports minimum); a freshly-airdropped account funded with only 1,000
/// lamports does not.
#[test]
fn rent_exempt_constraint_rejects_underfunded_account() {
    let provider = setup();

    build_check_rent_exempt(
        &provider,
        PROGRAM_ID,
        CheckRentExemptAccounts {
            target: provider.payer.address(),
        },
    )
    .send_and_confirm()
    .expect("the well-funded payer account must be accepted as rent-exempt");

    let underfunded = Keypair::new();
    provider
        .set_account_lamports(&underfunded.address(), 1_000)
        .expect("setting a tiny, deliberately-underfunded lamport balance should succeed");

    let underfunded_result = build_check_rent_exempt(
        &provider,
        PROGRAM_ID,
        CheckRentExemptAccounts {
            target: underfunded.address(),
        },
    )
    .send_and_confirm();
    // `CheckRentExempt { target }` â€” `target` is field index 0 ->
    // 3000 + 0*100 + ConstraintRentExempt(5) = 3005.
    assert_custom_code(underfunded_result, 3005);
}

/// `close`'s self-close guard: passing the *same* address for both the
/// account being closed and its own destination must be rejected with
/// `NaclacError::ConstraintClose`, unconditionally, before any other close
/// logic (lamport transfer, data wipe, reassignment) runs. Every existing
/// `close` test (`close_vault`) only exercises the success path with a
/// distinct destination.
#[test]
fn close_rejects_self_close() {
    let provider = setup();

    let target = Keypair::new();

    let result = build_close_vault_self(
        &provider,
        PROGRAM_ID,
        CloseVaultSelfAccounts {
            target: target.address(),
            destination: target.address(),
        },
    )
    .send_and_confirm();
    // `CloseVaultSelf { target, destination }` â€” the self-close guard
    // (`close_account.rs`, target == dest check) fires on `target`, field
    // index 0 -> 3000 + 0*100 + ConstraintClose(26) = 3026.
    assert_custom_code(result, 3026);
}

/// Relation constraint custom error (`field @ CustomError` syntax):
/// `admin = authority @ VaultError::WrongAdmin` replaces the default
/// `NaclacError::Unauthorized` with the custom error's own code on a
/// mismatch â€” never exercised elsewhere, since `related_vault` only takes
/// the default-error path.
#[test]
fn relation_constraint_custom_error_replaces_default_unauthorized() {
    let provider = setup();
    let (vault_pda, _bump) = get_vault_pda(&PROGRAM_ID);

    build_init_vault(
        &provider,
        PROGRAM_ID,
        InitVaultAccounts {
            payer: provider.payer.address(),
            vault: vault_pda,
            system_program: Address::default(),
        },
    )
    .send_and_confirm()
    .expect("init_vault should succeed");

    let stranger = Keypair::new();
    let mismatched_result = build_related_vault_custom_error(
        &provider,
        PROGRAM_ID,
        RelatedVaultCustomErrorAccounts {
            vault: vault_pda,
            authority: stranger.address(),
        },
    )
    .signer(&stranger)
    .send_and_confirm();
    // `admin = authority @ VaultError::WrongAdmin` â€” the `@` custom-error
    // suffix replaces the default `NaclacError::Unauthorized` with
    // `VaultError::WrongAdmin`'s own code. `#[error_code]` offsets custom
    // discriminants by 6000 (`naclac-macros/src/error_code.rs`); with no
    // explicit discriminant, `WrongAdmin` (the enum's only variant) lands
    // at exactly 6000 â€” a genuinely different codespace from
    // `NaclacError`'s 3000s, proving the custom-error path fired instead
    // of the default (which would have been 3021, as in
    // `relation_constraint_rejects_mismatched_admin` above).
    assert_custom_code(mismatched_result, 6000);

    build_related_vault_custom_error(
        &provider,
        PROGRAM_ID,
        RelatedVaultCustomErrorAccounts {
            vault: vault_pda,
            authority: provider.payer.address(),
        },
    )
    .send_and_confirm()
    .expect("related_vault_custom_error must succeed when `authority` matches `vault.admin`");
}

/// `seeds::program`: derives/validates a PDA against a program ID other
/// than the current program â€” here the real, well-known System Program
/// ID. The correctly-derived PDA (matching bump) must be accepted; a
/// deliberately wrong bump must be rejected with
/// `NaclacError::ConstraintSeeds`, the same as an on-program PDA would be.
#[test]
fn seeds_program_constraint_derives_pda_against_external_program() {
    let provider = setup();

    let seed: &[u8] = b"external_pda";
    let (external_pda, external_bump) =
        Address::find_program_address(&[seed], &SYSTEM_PROGRAM_ID);

    build_check_external_pda(
        &provider,
        PROGRAM_ID,
        external_bump,
        CheckExternalPdaAccounts {
            target: external_pda,
        },
    )
    .send_and_confirm()
    .expect(
        "a PDA correctly derived against the external (System) program with the right bump must be accepted",
    );

    let wrong_bump = external_bump.wrapping_sub(1);
    let wrong_result = build_check_external_pda(
        &provider,
        PROGRAM_ID,
        wrong_bump,
        CheckExternalPdaAccounts {
            target: external_pda,
        },
    )
    .send_and_confirm();
    // `CheckExternalPda { target }` â€” `target` is field index 0 ->
    // 3000 + 0*100 + ConstraintSeeds(6) = 3006.
    assert_custom_code(wrong_result, 3006);
}

/// Real end-to-end proof that `init` on a Borsh `#[component]` with a
/// `#[max_len]` `Vec`/`String` field allocates the real serialized size
/// (`Note::SPACE`) by default, not `size_of::<Note>()` (naclac-macros gap
/// #3). Writes a 190-byte name (close to the 200-byte `#[max_len]` cap) with
/// no explicit `space =` on the `init` â€” this only succeeds if the account
/// was allocated with real room for the string, not the much smaller
/// in-memory `String` pointer/len/cap representation.
#[test]
fn init_note_allocates_real_max_len_space_not_in_memory_struct_size() {
    let provider = setup();
    let (note_pda, _bump) = get_note_pda(&PROGRAM_ID);

    let name = "x".repeat(190);

    build_init_note(
        &provider,
        PROGRAM_ID,
        name.clone(),
        InitNoteAccounts {
            payer: provider.payer.address(),
            note: note_pda,
            system_program: Address::default(),
        },
    )
    .send_and_confirm()
    .expect(
        "init_note with a 190-byte name and no explicit space= must succeed \
         if the default space computation uses Note::SPACE",
    );

    let note = fetch_note(&provider, &note_pda).expect("note should be readable");
    assert_eq!(note.name, name);
}

/// Real end-to-end proof that `init` handles a target account that already
/// holds lamports (e.g. a PDA that received a plain SOL transfer before
/// this instruction ran) â€” previously `create_account_signed` always used
/// the raw `CreateAccount` System instruction, which requires a
/// zero-lamport target and fails with `AccountAlreadyInUse` otherwise (see
/// `naclac-macros/docs/derive-accounts-gaps-audit.md`'s gap #4). Pre-funds
/// `seeded_pda` directly (a plain SOL transfer, not via the program at all)
/// before calling `init_seeded` against it.
#[test]
fn init_succeeds_against_a_pre_funded_pda() {
    let provider = setup();
    let (seeded_pda, _bump) = get_seeded_pda(&PROGRAM_ID);

    transfer_sol(&provider, &seeded_pda, 1_000_000)
        .expect("pre-funding the PDA with a plain SOL transfer should succeed");

    build_init_seeded(
        &provider,
        PROGRAM_ID,
        InitSeededAccounts {
            payer: provider.payer.address(),
            seeded: seeded_pda,
            system_program: Address::default(),
        },
    )
    .send_and_confirm()
    .expect("init should succeed against a pre-funded PDA, not fail with AccountAlreadyInUse");
}
