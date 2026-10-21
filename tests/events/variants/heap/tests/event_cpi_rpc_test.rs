use naclac_client::*;
use events_borsh_client::{
    instructions::{build_event_cpi_emit, EventCpiEmitAccounts},
    types::{CounterIncremented, PROGRAM_ID},
};

/// Runs `emit_cpi!` against devnet with the `events_borsh` program deployed
/// there at `PROGRAM_ID`, so the inner instructions come from
/// `getTransaction`'s wire format instead of litesvm. The payer wallet must
/// hold devnet SOL.
#[test]
#[ignore = "needs network access, a funded devnet wallet, and the events_borsh program deployed on devnet"]
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
        assert_eq!(inner.len(), 1, "{label}: {inner:?}");
        assert_eq!(inner[0].outer_index, 0, "{label}");
        assert_eq!(inner[0].stack_height, 2, "{label}");
        assert_eq!(inner[0].program_id, PROGRAM_ID, "{label}");
        assert_eq!(inner[0].accounts, vec![event_authority], "{label}");

        #[cfg(feature = "borsh")]
        let decoded: Vec<CounterIncremented> = meta
            .parse_events_borsh()
            .expect("parse_events_borsh must not fail");
        #[cfg(feature = "wincode")]
        let decoded: Vec<CounterIncremented> = meta
            .parse_events_wincode()
            .expect("parse_events_wincode must not fail");
        assert_eq!(decoded.len(), 1, "{label}");
        assert_eq!(decoded[0].new_count, 7, "{label}");
    }
}
