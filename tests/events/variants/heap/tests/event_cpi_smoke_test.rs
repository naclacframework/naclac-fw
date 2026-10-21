use naclac_client::*;
use events_borsh_client::{
    instructions::{build_event_cpi_smoke, EventCpiSmokeAccounts},
    types::PROGRAM_ID,
};

const EVENT_DISPATCH_TAG: [u8; 8] = [34, 131, 189, 8, 189, 141, 224, 48];

fn setup() -> NaclacProvider {
    let payer = load_node_wallet().expect("Failed to load local Solana keypair");
    NaclacProvider::new("litesvm", payer).expect("Failed to construct NaclacProvider")
}

fn event_authority() -> Address {
    Address::find_program_address(&[b"__event_authority"], &PROGRAM_ID).0
}

fn tag_payload() -> Vec<u8> {
    let mut data = EVENT_DISPATCH_TAG.to_vec();
    data.extend_from_slice(&[1, 2, 3, 4]);
    data
}

fn error_text<T: std::fmt::Debug, E: std::fmt::Debug>(result: Result<T, E>) -> String {
    format!("{:?}", result.expect_err("expected the transaction to fail"))
}

/// `#[event_cpi]`'s auto-injected accounts carry real, enforced constraints,
/// and the handler's signed self-CPI carrying the reserved tag is accepted
/// by the receive arm.
#[test]
fn self_cpi_with_the_real_authority_succeeds_and_wrong_accounts_are_rejected() {
    let provider = setup();

    let meta = build_event_cpi_smoke(
        &provider,
        PROGRAM_ID,
        EventCpiSmokeAccounts {
            event_authority: event_authority(),
            program: PROGRAM_ID,
        },
    )
    .send_and_confirm()
    .expect("event_cpi_smoke must accept the real event_authority PDA");
    assert!(
        meta.logs.iter().any(|line| line.contains("invoke [2]")),
        "the handler's self-CPI must have reached the receive arm: {:?}",
        meta.logs
    );

    assert!(build_event_cpi_smoke(
        &provider,
        PROGRAM_ID,
        EventCpiSmokeAccounts {
            event_authority: provider.payer.address(),
            program: PROGRAM_ID,
        },
    )
    .send_and_confirm()
    .is_err());

    assert!(build_event_cpi_smoke(
        &provider,
        PROGRAM_ID,
        EventCpiSmokeAccounts {
            event_authority: event_authority(),
            program: provider.payer.address(),
        },
    )
    .send_and_confirm()
    .is_err());
}

/// The receive arm must reject a top-level (or foreign-CPI) call that
/// carries the reserved tag but lacks the real, signing event authority.
#[test]
fn receive_arm_rejects_missing_unsigned_and_foreign_signer_authority() {
    let provider = setup();

    let no_accounts = error_text(
        InstructionBuilder::new(&provider, PROGRAM_ID, tag_payload()).send_and_confirm(),
    );
    assert!(no_accounts.contains("Custom(10016)"), "{no_accounts}");

    let unsigned = error_text(
        InstructionBuilder::new(&provider, PROGRAM_ID, tag_payload())
            .account(
                AccountMeta {
                    address: event_authority(),
                    is_signer: false,
                    is_writable: false,
                },
                "event_authority",
            )
            .send_and_confirm(),
    );
    assert!(unsigned.contains("Custom(10002)"), "{unsigned}");

    let foreign_signer = error_text(
        InstructionBuilder::new(&provider, PROGRAM_ID, tag_payload())
            .account(
                AccountMeta {
                    address: provider.payer.address(),
                    is_signer: true,
                    is_writable: false,
                },
                "event_authority",
            )
            .send_and_confirm(),
    );
    assert!(foreign_signer.contains("Custom(10006)"), "{foreign_signer}");
}
