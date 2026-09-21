use naclac_client::*;
use events_client::{
    instructions::{build_emit_via_self_cpi_signed_baseline, EmitViaSelfCpiSignedBaselineAccounts},
    types::PROGRAM_ID,
};

fn setup() -> NaclacProvider {
    let payer = load_node_wallet().expect("Failed to load local Solana keypair");
    NaclacProvider::new("litesvm", payer).expect("Failed to construct NaclacProvider")
}

/// Real, measured CU cost of a *security-correct* self-CPI â€” signs the
/// `event_authority` PDA via `invoke_signed` (Anchor's `emit_cpi!` pattern),
/// unlike `emit_via_self_cpi_baseline`'s plain unsigned `invoke`. This is
/// the number a real `emit_cpi!` feature would actually pay, since the
/// unsigned version is spoofable (anyone could call `log_event` directly).
#[test]
fn signed_self_cpi_baseline_cu_cost() {
    let provider = setup();
    let (event_authority, _bump) =
        Address::find_program_address(&[b"__event_authority"], &PROGRAM_ID);

    let signed = build_emit_via_self_cpi_signed_baseline(
        &provider,
        PROGRAM_ID,
        EmitViaSelfCpiSignedBaselineAccounts {
            event_authority,
            program: PROGRAM_ID,
        },
    )
    .log()
    .send_and_confirm()
    .expect("emit_via_self_cpi_signed_baseline should succeed");

    println!(
        "[events (pinocchio)] signed self-CPI (8-byte payload): {} CU",
        signed.compute_units_consumed
    );
}
