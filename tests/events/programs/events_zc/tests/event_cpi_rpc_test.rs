use naclac_client::*;
use events_zc_client::{
    instructions::{build_event_cpi_emit, EventCpiEmitAccounts},
    types::{CounterIncremented, SizedPayload, PROGRAM_ID},
};

/// Runs `emit_cpi!` against devnet with the `events_zc` program deployed there
/// at `PROGRAM_ID`, so the inner instructions come from `getTransaction`'s
/// wire format instead of litesvm. The payer wallet must hold devnet SOL.
#[test]
#[ignore = "needs network access, a funded devnet wallet, and the events_zc program deployed on devnet"]
fn emit_cpi_events_decode_from_a_real_rpc_node() {
    let payer = load_node_wallet().expect("Failed to load local Solana keypair");
    let provider =
        NaclacProvider::new("devnet", payer).expect("Failed to construct NaclacProvider");
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
    .expect("event_cpi_emit should succeed on a real node");

    for (label, meta) in [
        ("send_and_confirm", meta.clone()),
        (
            "get_transaction",
            provider
                .get_transaction(&meta.signature)
                .expect("get_transaction must find the confirmed transaction"),
        ),
    ] {
        let inner = meta
            .inner_instructions
            .as_ref()
            .unwrap_or_else(|| panic!("{label}: the node must supply inner instructions"));
        assert_eq!(inner.len(), 2, "{label}: {inner:?}");
        for call in inner {
            assert_eq!(call.outer_index, 0, "{label}");
            assert_eq!(call.stack_height, 2, "{label}");
            assert_eq!(call.program_id, PROGRAM_ID, "{label}");
            assert_eq!(call.accounts, vec![event_authority], "{label}");
        }

        let counters: Vec<CounterIncremented> = meta
            .parse_events_zero_copy()
            .expect("parse_events_zero_copy must not fail");
        assert_eq!(counters.len(), 1, "{label}");
        assert_eq!(counters[0].new_count, 7, "{label}");

        let payloads: Vec<SizedPayload> = meta
            .parse_events_alloc()
            .expect("parse_events_alloc must not fail");
        assert_eq!(payloads.len(), 1, "{label}");
        assert_eq!(payloads[0].data, vec![1u8, 2, 3], "{label}");
    }
}
