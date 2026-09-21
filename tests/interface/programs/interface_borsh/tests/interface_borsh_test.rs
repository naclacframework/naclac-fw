use naclac_client::*;
use interface_borsh_client::{
    instructions::{build_check_token_interface, CheckTokenInterfaceAccounts},
    types::PROGRAM_ID,
};

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

fn setup() -> NaclacProvider {
    let payer = load_node_wallet().expect("Failed to load local Solana keypair");
    NaclacProvider::new("litesvm", payer).expect("Failed to construct NaclacProvider")
}

/// Same three cases as `programs/interface`'s test â€” see that file for the
/// full rationale. `interface.rs`'s only branch is
/// `#[cfg(feature = "pinocchio")]` vs. not, so this solana-borsh run
/// exercises the exact same code path a solana-zerocopy run would (hence no
/// dedicated `interface_zc` variant â€” see TEST_PLAN.md).
#[test]
fn real_token_program_is_accepted() {
    let provider = setup();

    let result = build_check_token_interface(
        &provider,
        PROGRAM_ID,
        CheckTokenInterfaceAccounts {
            token_program: TOKEN_PROGRAM_ID,
        },
    )
    .send_and_confirm();

    assert!(
        result.is_ok(),
        "the real SPL Token program should be accepted by Interface<TokenInterface>: {:?}",
        result.err()
    );
}

#[test]
fn real_token_2022_program_is_accepted() {
    let provider = setup();

    let result = build_check_token_interface(
        &provider,
        PROGRAM_ID,
        CheckTokenInterfaceAccounts {
            token_program: TOKEN_2022_PROGRAM_ID,
        },
    )
    .send_and_confirm();

    assert!(
        result.is_ok(),
        "the real Token-2022 program should be accepted by Interface<TokenInterface>: {:?}",
        result.err()
    );
}

#[test]
fn non_token_executable_program_is_rejected_with_program_id_mismatch() {
    let provider = setup();

    let result = build_check_token_interface(
        &provider,
        PROGRAM_ID,
        CheckTokenInterfaceAccounts {
            token_program: SYSTEM_PROGRAM_ID,
        },
    )
    .send_and_confirm();

    assert_custom_code(result, 3009);
}
