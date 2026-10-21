use naclac_client::*;
use error_codes_client::types::PROGRAM_ID;

const EVENT_DISPATCH_TAG: [u8; 8] = [34, 131, 189, 8, 189, 141, 224, 48];

/// A program with no `#[event_cpi]` struct must treat the reserved tag as an
/// unknown instruction — the receive arm is dead unless some handler opts in.
#[test]
fn reserved_tag_is_an_unknown_instruction_when_no_handler_uses_event_cpi() {
    let payer = load_node_wallet().expect("Failed to load local Solana keypair");
    let provider =
        NaclacProvider::new("litesvm", payer).expect("Failed to construct NaclacProvider");

    let mut data = EVENT_DISPATCH_TAG.to_vec();
    data.extend_from_slice(&[1, 2, 3, 4]);
    let err = InstructionBuilder::new(&provider, PROGRAM_ID, data)
        .send_and_confirm()
        .expect_err("the reserved tag must not dispatch without #[event_cpi]");
    let text = format!("{err:?}");
    assert!(text.contains("Custom(10012)"), "{text}");
}
