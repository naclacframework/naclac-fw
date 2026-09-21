use naclac_client::*;
use events_zc_client::{
    get_counter_pda,
    instructions::{
        build_emit_batch_alloc, build_emit_batch_fixed, build_increment_counter,
        build_init_counter, build_touch_counter_explicit_bump, EmitBatchAllocAccounts,
        EmitBatchFixedAccounts, IncrementCounterAccounts, InitCounterAccounts,
        TouchCounterExplicitBumpAccounts,
    },
    types::PROGRAM_ID,
    types::{BatchTouched, BatchTouchedAlloc, CounterIncremented},
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
    assert_eq!(events_1[0].new_count, 1);

    // `increment_counter` takes no args and hits the same account both
    // times, so without a fresh blockhash the second call would be a
    // byte-identical transaction to the first â€” litesvm (correctly)
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
    assert_eq!(
        events_2[0].new_count, 2,
        "the event payload must reflect the account's actual updated state, not a stale/cached value"
    );
}

/// Explicit `bump = counter.bump` (a self-reference on an existing account)
/// â€” verifying this actually works rather than assuming it does, since no
/// existing test exercises this exact form. Reasoned to be equivalent to
/// bare `bump`'s proven auto-path (see `touch_counter_explicit_bump.rs`),
/// but reasoning isn't verification: this test is the verification.
#[test]
fn explicit_self_referencing_bump_on_existing_account_works() {
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

    build_touch_counter_explicit_bump(
        &provider,
        PROGRAM_ID,
        TouchCounterExplicitBumpAccounts { counter: counter_pda },
    )
    .send_and_confirm()
    .expect(
        "touch_counter_explicit_bump must succeed â€” `bump = counter.bump` should validate \
         identically to bare `bump`'s auto-path, not fail some self-reference-specific gap",
    );
}

/// Verifies `#[event(alloc)]` end-to-end against the client-gen pipeline this
/// mode was never originally designed for: naclac-syn's IDL generation (no
/// bogus zero-copy padding field, correct `vec`/`string` IDL types) and
/// naclac-client-gen's Rust codegen (a plain, non-`Pod` struct plus a
/// generated `NaclacAllocEvent::decode` sequential reader), decoded via the
/// new `parse_events_alloc`. Emits a plain fixed-size `#[event]`
/// (`BatchTouched`, bytemuck-cast) and an `#[event(alloc)]`
/// (`BatchTouchedAlloc`, Vec/String) side by side so both paths are checked
/// in one place.
#[test]
fn alloc_and_fixed_events_round_trip() {
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

    let tx_fixed = build_emit_batch_fixed(
        &provider,
        PROGRAM_ID,
        EmitBatchFixedAccounts { counter: counter_pda },
    )
    .send_and_confirm()
    .expect("emit_batch_fixed should succeed");

    let fixed_events: Vec<BatchTouched> = tx_fixed
        .parse_events_zero_copy()
        .expect("failed to parse BatchTouched from the fixed-event transaction's logs");
    assert_eq!(fixed_events.len(), 1);
    assert_eq!(fixed_events[0].tag, 42);
    assert_eq!(fixed_events[0].values, [1, 2, 3, 4, 5, 6, 7, 8]);

    if let ClientBackend::LiteSVM(svm) = &provider.backend {
        svm.lock().unwrap().expire_blockhash();
    }

    let tx_alloc = build_emit_batch_alloc(
        &provider,
        PROGRAM_ID,
        EmitBatchAllocAccounts { counter: counter_pda },
    )
    .send_and_confirm()
    .expect("emit_batch_alloc should succeed");

    let alloc_events: Vec<BatchTouchedAlloc> = tx_alloc
        .parse_events_alloc()
        .expect("failed to parse BatchTouchedAlloc from the alloc-event transaction's logs");
    assert_eq!(alloc_events.len(), 1);
    assert_eq!(alloc_events[0].tag, 42);
    assert_eq!(alloc_events[0].values, vec![1, 2, 3, 4, 5, 6, 7, 8]);
    assert_eq!(alloc_events[0].label, "batch");
}
