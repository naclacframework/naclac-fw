use naclac_client::*;
use events_client::{
    instructions::{
        build_event_cpi_emit, build_event_cpi_emit_nested, EventCpiEmitAccounts,
        EventCpiEmitNestedAccounts,
    },
    types::{CounterIncremented, NestedPadded, SizedPayload, PROGRAM_ID},
};

const EVENT_DISPATCH_TAG: [u8; 8] = [34, 131, 189, 8, 189, 141, 224, 48];

/// `emit_cpi!` must put `tag ++ event discriminator ++ fields` into a real
/// signed self-CPI whose sole signer is the `event_authority` PDA — for a
/// plain zero-copy `#[event]` and for an `#[event(alloc)]` one.
#[test]
fn emit_cpi_records_both_event_encodings_in_the_inner_instruction_trace() {
    let payer = load_node_wallet().expect("Failed to load local Solana keypair");
    let provider =
        NaclacProvider::new("litesvm", payer).expect("Failed to construct NaclacProvider");
    let event_authority = Address::find_program_address(&[b"__event_authority"], &PROGRAM_ID).0;

    let meta = build_event_cpi_emit(
        &provider,
        PROGRAM_ID,
        EventCpiEmitAccounts {
            event_authority,
            program: PROGRAM_ID,
        },
    )
    .send_and_confirm()
    .expect("event_cpi_emit should succeed");

    let inner = meta
        .inner_instructions
        .as_ref()
        .expect("litesvm must supply inner instructions");
    assert_eq!(inner.len(), 2, "two emit_cpi! calls must be two inner instructions: {inner:?}");
    for call in inner {
        assert_eq!(call.outer_index, 0);
        assert_eq!(call.stack_height, 2);
        assert_eq!(call.program_id, PROGRAM_ID);
        assert_eq!(call.accounts, vec![event_authority]);
        assert_eq!(call.data[..EVENT_DISPATCH_TAG.len()], EVENT_DISPATCH_TAG);
    }

    let (discriminator, fields) = inner[0].data[EVENT_DISPATCH_TAG.len()..]
        .split_at(CounterIncremented::DISCRIMINATOR.len());
    assert_eq!(discriminator, CounterIncremented::DISCRIMINATOR);
    assert_eq!(fields, 7u64.to_le_bytes());

    let (discriminator, fields) = inner[1].data[EVENT_DISPATCH_TAG.len()..]
        .split_at(SizedPayload::DISCRIMINATOR.len());
    assert_eq!(discriminator, SizedPayload::DISCRIMINATOR);
    assert_eq!(fields, [3, 0, 0, 0, 1, 2, 3]);

    // The public decode API must find both CPI-emitted events on its own —
    // this program logs nothing, so every result here came from an inner
    // instruction.
    let counters: Vec<CounterIncremented> = meta
        .parse_events_zero_copy()
        .expect("parse_events_zero_copy must not fail");
    assert_eq!(counters.len(), 1);
    let counted = counters[0].new_count;
    assert_eq!(counted, 7);

    let payloads: Vec<SizedPayload> = meta
        .parse_events_alloc()
        .expect("parse_events_alloc must not fail");
    assert_eq!(payloads.len(), 1);
    assert_eq!(payloads[0].data, vec![1u8, 2, 3]);
}

/// A nested `defined_type` with internal padding keeps its C layout on the
/// wire inside an `alloc` event, and the client decodes it back.
#[test]
fn emit_cpi_alloc_event_carries_a_padded_defined_type_in_c_layout() {
    let payer = load_node_wallet().expect("Failed to load local Solana keypair");
    let provider =
        NaclacProvider::new("litesvm", payer).expect("Failed to construct NaclacProvider");
    let event_authority = Address::find_program_address(&[b"__event_authority"], &PROGRAM_ID).0;

    let meta = build_event_cpi_emit_nested(
        &provider,
        PROGRAM_ID,
        EventCpiEmitNestedAccounts {
            event_authority,
            program: PROGRAM_ID,
        },
    )
    .send_and_confirm()
    .expect("event_cpi_emit_nested should succeed");

    let inner = meta
        .inner_instructions
        .as_ref()
        .expect("litesvm must supply inner instructions");
    assert_eq!(inner.len(), 1);

    let padded = |kind: u8, amount: u64| {
        let mut bytes = vec![kind, 0, 0, 0, 0, 0, 0, 0];
        bytes.extend_from_slice(&amount.to_le_bytes());
        bytes
    };
    let mut expected_fields = vec![5u8];
    expected_fields.extend(padded(3, 0x1122_3344_5566_7788));
    expected_fields.extend(2u32.to_le_bytes());
    expected_fields.extend(padded(9, 42));
    expected_fields.extend(padded(3, 0x1122_3344_5566_7788));
    expected_fields.extend(2u32.to_le_bytes());
    expected_fields.extend(b"hi");
    let fields = &inner[0].data[EVENT_DISPATCH_TAG.len() + NestedPadded::DISCRIMINATOR.len()..];
    assert_eq!(fields, expected_fields);

    let decoded: Vec<NestedPadded> = meta
        .parse_events_alloc()
        .expect("parse_events_alloc must not fail");
    assert_eq!(decoded.len(), 1);
    assert_eq!(decoded[0].head, 5);
    assert_eq!(decoded[0].inner.kind, 3);
    let inner_amount = decoded[0].inner.amount;
    assert_eq!(inner_amount, 0x1122_3344_5566_7788);
    assert_eq!(decoded[0].many.len(), 2);
    assert_eq!(decoded[0].many[0].kind, 9);
    let first_amount = decoded[0].many[0].amount;
    let second_amount = decoded[0].many[1].amount;
    assert_eq!(first_amount, 42);
    assert_eq!(second_amount, 0x1122_3344_5566_7788);
    assert_eq!(decoded[0].note, "hi");
}
