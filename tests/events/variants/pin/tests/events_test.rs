use naclac_client::*;
use events_client::{
    get_counter_pda,
    instructions::{
        build_emit_roster, build_increment_counter, build_init_counter, EmitRosterAccounts,
        IncrementCounterAccounts, InitCounterAccounts,
    },
    types::PROGRAM_ID,
    types::{CounterIncremented, RosterChanged, RosterChangedAlloc},
};

fn setup() -> NaclacProvider {
    let payer = load_node_wallet().expect("Failed to load local Solana keypair");
    NaclacProvider::new("litesvm", payer).expect("Failed to construct NaclacProvider")
}

/// Emits a real event via `emit!`, decodes it back off the transaction's
/// actual log output (not a mocked/hand-constructed log line), and confirms
/// the field value round-trips correctly and the discriminator the
/// generated client expects (`sha256("event:CounterIncremented")[..8]`,
/// computed independently inside `parse_events_zero_copy` itself) actually
/// matches what the on-chain `emit_event` call produced.
#[test]
fn event_round_trips_correctly_across_multiple_emissions() {
    let provider = setup();
    let (counter_pda, _bump) = get_counter_pda(&PROGRAM_ID);

    build_init_counter(
        &provider,
        PROGRAM_ID,
        InitCounterAccounts {
            payer: provider.payer.address(),
            counter: counter_pda,
            system_program: Address::default(),
        },
    )
    .send_and_confirm()
    .expect("init_counter should succeed");

    let tx_meta_1 = build_increment_counter(
        &provider,
        PROGRAM_ID,
        IncrementCounterAccounts { counter: counter_pda },
    )
    .send_and_confirm()
    .expect("first increment_counter should succeed");

    let events_1: Vec<CounterIncremented> = tx_meta_1
        .parse_events_zero_copy()
        .expect("failed to parse CounterIncremented from the first transaction's logs");
    assert_eq!(events_1.len(), 1, "exactly one event must be captured per transaction");
    let first_count = events_1[0].new_count;
    assert_eq!(first_count, 1);

    // `increment_counter` takes no args and hits the same account both
    // times, so without a fresh blockhash the second call would be a
    // byte-identical transaction to the first — litesvm (correctly)
    // rejects that as a replay (`AlreadyProcessed`), not a naclac bug.
    if let ClientBackend::LiteSVM(svm) = &provider.backend {
        svm.lock().unwrap().expire_blockhash();
    }

    let tx_meta_2 = build_increment_counter(
        &provider,
        PROGRAM_ID,
        IncrementCounterAccounts { counter: counter_pda },
    )
    .send_and_confirm()
    .expect("second increment_counter should succeed");

    let events_2: Vec<CounterIncremented> = tx_meta_2
        .parse_events_zero_copy()
        .expect("failed to parse CounterIncremented from the second transaction's logs");
    assert_eq!(events_2.len(), 1);
    let second_count = events_2[0].new_count;
    assert_eq!(
        second_count, 2,
        "the event payload must reflect the account's actual updated state, not a stale/cached value"
    );
}

#[test]
fn events_with_a_fixed_vec_and_a_fixed_string_round_trip() {
    let provider = setup();
    let (counter_pda, _bump) = get_counter_pda(&PROGRAM_ID);

    build_init_counter(
        &provider,
        PROGRAM_ID,
        InitCounterAccounts {
            payer: provider.payer.address(),
            counter: counter_pda,
            system_program: Address::default(),
        },
    )
    .send_and_confirm()
    .expect("init_counter should succeed");

    let tx = build_emit_roster(
        &provider,
        PROGRAM_ID,
        EmitRosterAccounts { counter: counter_pda },
    )
    .send_and_confirm()
    .expect("emit_roster should succeed");

    let plain: Vec<RosterChanged> = tx
        .parse_events_zero_copy()
        .expect("failed to parse RosterChanged from the transaction's logs");
    assert_eq!(plain.len(), 1);
    let plain_scores = plain[0].scores;
    let plain_name = plain[0].name;
    assert_eq!(plain_scores.iter().collect::<Vec<u64>>(), vec![10, 20]);
    assert_eq!(plain_name.as_str().ok(), Some("roster"));

    let alloc: Vec<RosterChangedAlloc> = tx
        .parse_events_alloc()
        .expect("failed to parse RosterChangedAlloc from the transaction's logs");
    assert_eq!(alloc.len(), 1);
    assert_eq!(alloc[0].scores.iter().collect::<Vec<u64>>(), vec![10, 20]);
    assert_eq!(alloc[0].name.as_str().ok(), Some("roster"));
    assert_eq!(alloc[0].note, "alloc");
}
