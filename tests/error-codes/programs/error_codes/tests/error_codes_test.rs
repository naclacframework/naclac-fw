use naclac_client::*;
use error_codes_client::{
    instructions::{
        build_check_amount, build_check_authority, build_check_fixed_address,
        build_require_signer, CheckAmountAccounts, CheckAuthorityAccounts,
        CheckFixedAddressAccounts, RequireSignerAccounts,
    },
    types::PROGRAM_ID,
};

fn setup() -> NaclacProvider {
    let payer = load_node_wallet().expect("Failed to load local Solana keypair");
    NaclacProvider::new("litesvm", payer).expect("Failed to construct NaclacProvider")
}

/// Asserts a transaction failed with exactly the given `Custom` error code
/// â€” not just "any error", the specific numeric code `#[error_code]`
/// (custom errors, 6000+) or a `NaclacError` (framework errors, 3000s)
/// actually produces.
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

/// `#[error_code]`'s 6000 offset: with no explicit discriminants,
/// `TestError::ZeroAmount`/`Unauthorized`/`TooLarge` land at exactly
/// 6000/6001/6002 (`e as u32 + 6000`, confirmed directly from
/// `naclac-macros/src/error_code.rs`, not assumed).
#[test]
fn custom_error_codes_surface_correctly() {
    let provider = setup();

    build_check_amount(
        &provider,
        PROGRAM_ID,
        500,
        CheckAmountAccounts {
            payer: provider.payer.address(),
        },
    )
    .send_and_confirm()
    .expect("check_amount with a valid amount should succeed");

    let zero_result = build_check_amount(
        &provider,
        PROGRAM_ID,
        0,
        CheckAmountAccounts {
            payer: provider.payer.address(),
        },
    )
    .send_and_confirm();
    assert_custom_code(zero_result, 6000); // TestError::ZeroAmount

    let too_large_result = build_check_amount(
        &provider,
        PROGRAM_ID,
        1_000_001,
        CheckAmountAccounts {
            payer: provider.payer.address(),
        },
    )
    .send_and_confirm();
    assert_custom_code(too_large_result, 6002); // TestError::TooLarge

    let stranger = Keypair::new();
    let unauthorized_result = build_check_authority(
        &provider,
        PROGRAM_ID,
        CheckAuthorityAccounts {
            payer: provider.payer.address(),
            required: stranger.address(),
        },
    )
    .send_and_confirm();
    assert_custom_code(unauthorized_result, 6001); // TestError::Unauthorized

    build_check_authority(
        &provider,
        PROGRAM_ID,
        CheckAuthorityAccounts {
            payer: provider.payer.address(),
            required: provider.payer.address(),
        },
    )
    .send_and_confirm()
    .expect("check_authority should succeed when payer matches required");
}

/// Confirms `NaclacError` framework errors and `#[error_code]` custom
/// errors genuinely don't collide in code-space â€” not just that the two
/// formulas don't overlap on paper (3000s vs. 6000+), but that a real
/// framework-triggered failure actually lands in the 3000s as expected.
///
/// `address = <const>` is used rather than a missing `Signer` signature:
/// omitting a required signature never reaches the chain at all â€” Solana's
/// own client-side transaction-signing rules reject building the
/// transaction outright (`NotEnoughSigners`), so the on-chain
/// `ConstraintSigner` check never runs. A wrong `address` is a perfectly
/// valid, signable transaction that fails *on-chain*, which is what this
/// test is actually about.
#[test]
fn framework_errors_stay_in_their_own_codespace() {
    let provider = setup();

    build_check_fixed_address(
        &provider,
        PROGRAM_ID,
        CheckFixedAddressAccounts {
            target: SYSTEM_PROGRAM_ID,
        },
    )
    .send_and_confirm()
    .expect("check_fixed_address should succeed when target is the real System Program");

    let wrong_address_result = build_check_fixed_address(
        &provider,
        PROGRAM_ID,
        CheckFixedAddressAccounts {
            target: provider.payer.address(),
        },
    )
    .send_and_confirm();
    // `target` is field index 0 -> 3000 + 0*100 + ConstraintAddress(3) = 3003.
    assert_custom_code(wrong_address_result, 3003);
}

/// A missing `Signer` signature is caught even earlier than the on-chain
/// program: Solana's own client-side rules refuse to even construct a
/// transaction that's missing a signature for a declared signer. Worth
/// confirming as its own real property, distinct from the on-chain
/// `NaclacError::ConstraintSigner` check the framework also has as
/// defense-in-depth (which this specific path never actually reaches).
#[test]
fn missing_signer_signature_is_rejected_before_reaching_the_chain() {
    let provider = setup();

    let authority = Keypair::new();

    let missing_signature_result = build_require_signer(
        &provider,
        PROGRAM_ID,
        RequireSignerAccounts {
            authority: authority.address(),
        },
    )
    .send_and_confirm();
    assert!(
        missing_signature_result.is_err(),
        "a transaction missing a required signer's signature must not succeed"
    );

    build_require_signer(
        &provider,
        PROGRAM_ID,
        RequireSignerAccounts {
            authority: authority.address(),
        },
    )
    .signer(&authority)
    .send_and_confirm()
    .expect("require_signer should succeed once authority actually signs");
}
