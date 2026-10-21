use naclac_client::*;
use events_zc_client::{
    instructions::{
        build_emit_via_self_cpi_signed_baseline, build_event_cpi_emit_one, build_event_cpi_log_one,
        build_event_cpi_noop, EmitViaSelfCpiSignedBaselineAccounts, EventCpiEmitOneAccounts,
        EventCpiLogOneAccounts, EventCpiNoopAccounts,
    },
    types::PROGRAM_ID,
};

/// Real measured CU cost of `emit_cpi!` for one `CounterIncremented`, against
/// the same event emitted with plain `emit!` (a `sol_log_data` line) and an
/// instruction that emits nothing. All three share `EventCpiEmit`, so their
/// account validation cost is identical and the differences are the emission
/// itself. The hand-written signed self-CPI (`emit_via_self_cpi_signed_baseline`,
/// the pre-`emit_cpi!` prototype) is printed for reference.
#[test]
fn emit_cpi_cu_cost_against_log_emit_and_the_hand_written_baseline() {
    let payer = load_node_wallet().expect("Failed to load local Solana keypair");
    let provider =
        NaclacProvider::new("litesvm", payer).expect("Failed to construct NaclacProvider");
    let event_authority = Address::find_program_address(&[b"__event_authority"], &PROGRAM_ID).0;

    let noop = build_event_cpi_noop(
        &provider,
        PROGRAM_ID,
        EventCpiNoopAccounts {
            event_authority,
            program: PROGRAM_ID,
        },
    )
        .send_and_confirm()
        .expect("event_cpi_noop should succeed")
        .compute_units_consumed;
    let log = build_event_cpi_log_one(
        &provider,
        PROGRAM_ID,
        EventCpiLogOneAccounts {
            event_authority,
            program: PROGRAM_ID,
        },
    )
    .send_and_confirm()
        .expect("event_cpi_log_one should succeed")
        .compute_units_consumed;
    let cpi = build_event_cpi_emit_one(
        &provider,
        PROGRAM_ID,
        EventCpiEmitOneAccounts {
            event_authority,
            program: PROGRAM_ID,
        },
    )
    .send_and_confirm()
        .expect("event_cpi_emit_one should succeed")
        .compute_units_consumed;
    let baseline = build_emit_via_self_cpi_signed_baseline(
        &provider,
        PROGRAM_ID,
        EmitViaSelfCpiSignedBaselineAccounts {
            event_authority,
            program: PROGRAM_ID,
        },
    )
    .send_and_confirm()
    .expect("emit_via_self_cpi_signed_baseline should succeed")
    .compute_units_consumed;

    println!(
        "[events_zc] total CU — noop: {noop} | plain emit!: {log} | emit_cpi!: {cpi} | hand-written signed self-CPI: {baseline}"
    );
    println!(
        "[events_zc] emission overhead over noop — plain emit!: {} CU | emit_cpi!: {} CU",
        log - noop,
        cpi - noop
    );

    assert!(noop < log, "a log emission must cost more than nothing");
    assert!(log < cpi, "a self-CPI must cost more than a log line");
}
