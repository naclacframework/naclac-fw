use naclac_client::*;
use events_zc_client::{
    instructions::{build_emit_via_self_cpi_signed_baseline, EmitViaSelfCpiSignedBaselineAccounts},
    types::PROGRAM_ID,
};

fn load_program(provider: &NaclacProvider) {
    let mut so_path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    so_path.pop(); // programs
    so_path.pop(); // events workspace root
    so_path.push("target/deploy/events_zc.so");

    provider
        .add_program(&PROGRAM_ID, so_path.to_str().unwrap())
        .expect("Failed to load events_zc program binary");
}

fn setup() -> NaclacProvider {
    let payer = load_node_wallet().expect("Failed to load local Solana keypair");
    let provider = NaclacProvider::new("litesvm", payer);
    load_program(&provider);
    provider
}

/// Real, measured CU cost of a *security-correct* self-CPI — signs the
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
        "[events_zc] signed self-CPI (8-byte payload): {} CU",
        signed.compute_units_consumed
    );
}
