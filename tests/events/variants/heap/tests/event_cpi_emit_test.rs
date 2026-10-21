use naclac_client::*;
use events_borsh_client::{
    instructions::{build_event_cpi_emit, EventCpiEmitAccounts},
    types::{CounterIncremented, PROGRAM_ID},
};

const EVENT_DISPATCH_TAG: [u8; 8] = [34, 131, 189, 8, 189, 141, 224, 48];

/// `emit_cpi!` must put `tag ++ event discriminator ++ borsh fields` into a
/// real signed self-CPI whose sole signer is the `event_authority` PDA.
#[test]
fn emit_cpi_records_the_event_in_the_inner_instruction_trace() {
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
    assert_eq!(inner.len(), 1, "one emit_cpi! call must be one inner instruction: {inner:?}");
    let call = &inner[0];
    assert_eq!(call.outer_index, 0);
    assert_eq!(call.stack_height, 2);
    assert_eq!(call.program_id, PROGRAM_ID);
    assert_eq!(call.accounts, vec![event_authority]);

    let (tag, rest) = call.data.split_at(EVENT_DISPATCH_TAG.len());
    assert_eq!(tag, EVENT_DISPATCH_TAG);
    let (discriminator, fields) = rest.split_at(CounterIncremented::DISCRIMINATOR.len());
    assert_eq!(discriminator, CounterIncremented::DISCRIMINATOR);
    assert_eq!(fields, 7u64.to_le_bytes());

    // The public decode API must find the CPI-emitted event on its own —
    // this program logs nothing, so every result here came from the inner
    // instruction.
    #[cfg(feature = "borsh")]
    let decoded: Vec<CounterIncremented> = meta
        .parse_events_borsh()
        .expect("parse_events_borsh must not fail");
    #[cfg(feature = "wincode")]
    let decoded: Vec<CounterIncremented> = meta
        .parse_events_wincode()
        .expect("parse_events_wincode must not fail");
    assert_eq!(decoded.len(), 1);
    assert_eq!(decoded[0].new_count, 7);

    // Looking the confirmed transaction up again must yield the same events.
    let fetched = provider
        .get_transaction(&meta.signature)
        .expect("get_transaction must find the confirmed transaction");
    #[cfg(feature = "borsh")]
    let refetched: Vec<CounterIncremented> = fetched
        .parse_events_borsh()
        .expect("parse_events_borsh must not fail");
    #[cfg(feature = "wincode")]
    let refetched: Vec<CounterIncremented> = fetched
        .parse_events_wincode()
        .expect("parse_events_wincode must not fail");
    assert_eq!(refetched.len(), 1);
    assert_eq!(refetched[0].new_count, 7);
}
