use naclac_client::*;
use discriminator_client::{
    instructions::{
        build_init_config, build_init_vault, build_read_vault, InitConfigAccounts,
        InitVaultAccounts, ReadVaultAccounts,
    },
    get_config_pda, get_vault_pda,
    types::PROGRAM_ID,
};

/// The error code for a discriminator mismatch on the account at field index
/// 1: `10000 + 1 * 100 + InvalidAccountDiscriminator (22)`, in every layout.
const MISMATCHED_DISCRIMINATOR_CODE: u32 = 10122;

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

    // `ReadVault { caller, vault }` — `vault` is field index 1, not `mut`, so
    // `try_from` runs, not the discriminator-skipping `try_from_mut`.
    assert_custom_code(result, MISMATCHED_DISCRIMINATOR_CODE);
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
