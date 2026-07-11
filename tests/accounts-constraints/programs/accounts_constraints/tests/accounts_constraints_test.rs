use naclac_client::*;
use accounts_constraints_client::{
    fetch_ledger, fetch_vault,
    get_ledger_pda, get_seeded_pda, get_vault_pda,
    instructions::{
        build_check_address, build_check_owner, build_close_vault, build_init_if_needed_ledger,
        build_init_seeded, build_init_vault, build_related_vault, build_require_signer,
        build_touch_mut_vault, build_touch_seeded, CheckAddressAccounts, CheckOwnerAccounts,
        CloseVaultAccounts, InitIfNeededLedgerAccounts, InitSeededAccounts, InitVaultAccounts,
        RelatedVaultAccounts, RequireSignerAccounts, TouchMutVaultAccounts, TouchSeededAccounts,
    },
    types::PROGRAM_ID,
};

fn load_program(provider: &NaclacProvider) {
    let mut so_path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    so_path.pop(); // programs
    so_path.pop(); // accounts-constraints workspace root
    so_path.push("target/deploy/accounts_constraints.so");

    provider
        .add_program(&PROGRAM_ID, so_path.to_str().unwrap())
        .expect("Failed to load accounts_constraints program binary");
}

fn setup() -> NaclacProvider {
    let payer = load_node_wallet().expect("Failed to load local Solana keypair");
    let provider = NaclacProvider::new("litesvm", payer);
    load_program(&provider);
    provider
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
/// a second call with a *different* argument must not overwrite it — the
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
    assert!(
        wrong_owner_result.is_err(),
        "an account owned by our own program must be rejected when `owner` expects the System Program"
    );
}

/// `address`: rejects an account whose own key doesn't match the expected
/// constant — distinct from `owner` (checks the account's *own* key, not
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
    assert!(
        mismatched_result.is_err(),
        "address must reject an account whose own key doesn't match the expected constant"
    );
}

/// `mut`: the framework requires the account actually be marked writable on
/// the transaction. The generated SDK's typed builder always marks this
/// slot writable per the IDL, so the negative case is produced by taking
/// the built `InstructionBuilder` and flipping the raw `AccountMeta`
/// ourselves — proving the runtime check, not just the SDK's own honesty.
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
    assert!(
        result.is_err(),
        "the `mut` constraint must reject an account whose AccountMeta isn't actually writable, \
         independent of what the SDK would normally produce"
    );
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
    assert!(
        mismatched_result.is_err(),
        "related_vault must reject when `authority` doesn't match `vault.admin`"
    );

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
/// (`NaclacError::ConstraintSeeds`) — the on-chain
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
    assert!(
        wrong_result.is_err(),
        "touch_seeded must reject a wrong bump value, not just accept anything"
    );
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
    assert!(
        reuse_result.is_err(),
        "a closed account must not be usable as a `Vault` again in a later instruction"
    );
}
