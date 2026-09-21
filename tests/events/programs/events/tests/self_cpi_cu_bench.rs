use naclac_client::*;
use events_client::{
    instructions::{
        build_emit_via_self_cpi_baseline, build_emit_via_sol_log_data_baseline,
        build_no_cpi_baseline, EmitViaSelfCpiBaselineAccounts, EmitViaSolLogDataBaselineAccounts,
        NoCpiBaselineAccounts,
    },
    types::PROGRAM_ID,
};

fn setup() -> NaclacProvider {
    let payer = load_node_wallet().expect("Failed to load local Solana keypair");
    NaclacProvider::new("litesvm", payer).expect("Failed to construct NaclacProvider")
}

/// Real, measured CU cost of a bare self-CPI (program invoking itself with
/// zero accounts and a zero-length payload) under the pinocchio (no_std,
/// always zero-copy) backend, via litesvm's actual SVM execution â€” not an
/// estimate. Also runs a same-shaped no-CPI baseline (identical single
/// `address`-constrained account, no invoke) so the fixed CPI overhead can
/// be isolated from the surrounding instruction's own dispatch/validation
/// cost.
#[test]
fn self_cpi_baseline_cu_cost() {
    let provider = setup();

    let no_cpi = build_no_cpi_baseline(&provider, PROGRAM_ID, NoCpiBaselineAccounts { program: PROGRAM_ID })
        .log()
        .send_and_confirm()
        .expect("no_cpi_baseline should succeed");

    let self_cpi = build_emit_via_self_cpi_baseline(
        &provider,
        PROGRAM_ID,
        EmitViaSelfCpiBaselineAccounts { program: PROGRAM_ID },
    )
    .log()
    .send_and_confirm()
    .expect("emit_via_self_cpi_baseline should succeed");

    let sol_log = build_emit_via_sol_log_data_baseline(
        &provider,
        PROGRAM_ID,
        EmitViaSolLogDataBaselineAccounts { program: PROGRAM_ID },
    )
    .log()
    .send_and_confirm()
    .expect("emit_via_sol_log_data_baseline should succeed");

    println!(
        "[events (pinocchio)] no-CPI baseline: {} CU | self-CPI (empty payload): {} CU (isolated CPI overhead: {} CU) | sol_log_data emit (1 field CounterIncremented): {} CU (isolated emit overhead: {} CU)",
        no_cpi.compute_units_consumed,
        self_cpi.compute_units_consumed,
        self_cpi.compute_units_consumed as i64 - no_cpi.compute_units_consumed as i64,
        sol_log.compute_units_consumed,
        sol_log.compute_units_consumed as i64 - no_cpi.compute_units_consumed as i64,
    );
}
