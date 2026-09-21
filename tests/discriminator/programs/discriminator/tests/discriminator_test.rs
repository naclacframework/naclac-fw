use naclac_client::*;
use discriminator_client::{
    instructions::{
        build_init_config, build_init_vault, build_read_vault, InitConfigAccounts,
        InitVaultAccounts, ReadVaultAccounts,
    },
    get_config_pda, get_vault_pda,
    types::PROGRAM_ID,
};

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

/// The regression case for ZERO_COPY_BORSH_PARITY_AUDIT.md finding #1:
/// initialize a real `Config` account, then try to pass it into
/// `read_vault`'s `vault: Account<Vault>` slot. Both types are owned by the
/// same program, so the only thing that can reject this is the
/// discriminator check inside `Account::<Vault>::try_from`.
#[test]
fn rejects_type_confused_account_in_vault_slot() {
    let payer = load_node_wallet().expect("Failed to load local Solana keypair");
    let provider = NaclacProvider::new("litesvm", payer).expect("Failed to construct NaclacProvider");

    let (config_pda, _bump) = get_config_pda(&PROGRAM_ID);

    build_init_config(
        &provider,
        PROGRAM_ID,
        InitConfigAccounts {
            payer: provider.payer.address(),
            config: config_pda,
            system_program: Address::default(),
        },
    )
    .send_and_confirm()
    .expect("init_config should succeed");

    let result = build_read_vault(
        &provider,
        PROGRAM_ID,
        ReadVaultAccounts {
            caller: provider.payer.address(),
            vault: config_pda,
        },
    )
    .send_and_confirm();

    // `ReadVault { caller, vault }` â€” `vault` is field index 1, not `mut`
    // (so `try_from`, not the discriminator-skipping `try_from_mut`, is
    // used â€” `accounts.rs`). This is the pinocchio/zero-copy backend, so
    // `Account<Vault>` resolves to its zero-copy branch
    // (`wrappers/accounts/account.rs`'s `try_from`), whose discriminator
    // mismatch emits `InvalidAccountDiscriminator` (22) -> 3000 + 1*100 + 22 = 3122.
    assert_custom_code(result, 3122);
}

/// Positive-path companion: a correctly-typed Vault account must still be
/// accepted by the exact same instruction. Without this, the negative test
/// above could pass for the wrong reason (e.g. `read_vault` rejecting
/// everything).
#[test]
fn accepts_correctly_typed_vault_account() {
    let payer = load_node_wallet().expect("Failed to load local Solana keypair");
    let provider = NaclacProvider::new("litesvm", payer).expect("Failed to construct NaclacProvider");

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

    let result = build_read_vault(
        &provider,
        PROGRAM_ID,
        ReadVaultAccounts {
            caller: provider.payer.address(),
            vault: vault_pda,
        },
    )
    .send_and_confirm();

    assert!(
        result.is_ok(),
        "read_vault should succeed on a correctly-typed Vault account: {:?}",
        result.err()
    );
}
