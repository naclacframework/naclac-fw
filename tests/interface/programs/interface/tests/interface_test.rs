use naclac_client::*;
use interface_client::{
    instructions::{build_check_token_interface, CheckTokenInterfaceAccounts},
    types::PROGRAM_ID,
};

fn load_program(provider: &NaclacProvider) {
    let mut so_path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    so_path.pop(); // programs
    so_path.pop(); // interface workspace root
    so_path.push("target/deploy/interface.so");

    provider
        .add_program(&PROGRAM_ID, so_path.to_str().unwrap())
        .expect("Failed to load interface program binary");
}

/// Asserts a transaction failed with exactly the given `Custom` error code —
/// mirrors `tests/error-codes/programs/error_codes/tests/error_codes_test.rs`'s
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

fn setup() -> NaclacProvider {
    let payer = load_node_wallet().expect("Failed to load local Solana keypair");
    let provider = NaclacProvider::new("litesvm", payer);
    load_program(&provider);
    provider
}

/// `Interface<TokenInterface>`'s `NaclacAccount::try_from` (interface.rs) is
/// supposed to accept *either* well-known token program in the same field
/// with no per-mint branching required in the instruction body — that's the
/// entire point of the wrapper (`examples/token-vault`'s `deposit.rs`/
/// `withdraw.rs` both declare `token_program: Interface<TokenInterface>`
/// for exactly this reason). litesvm preloads the real SPL Token program
/// and the real Token-2022 program by default (confirmed in
/// `tests/token/TEST_PLAN.md`'s note on `litesvm-0.12.0/src/programs/mod.rs`),
/// so both can be exercised here without deploying anything extra.
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

/// A genuinely different, genuinely executable program (the real System
/// Program — confirmed executable in `tests/accounts-constraints`'s
/// `check_executable` test) must still be rejected: it passes the
/// `ConstraintExecutable` gate but fails the allowed-ids membership check,
/// which `interface.rs`'s `try_from` (both backends) reports as
/// `NaclacError::ProgramIdMismatch`, not `ConstraintAddress` — confirmed by
/// reading the wrapper directly rather than assumed. `token_program` is
/// field index 0 -> 3000 + 0*100 + ProgramIdMismatch(9) = 3009.
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
