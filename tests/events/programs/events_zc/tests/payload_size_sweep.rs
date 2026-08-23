use naclac_client::*;
use events_zc_client::{
    instructions::{
        build_emit_via_self_cpi_sized, build_emit_via_sol_log_data_sized,
        EmitViaSelfCpiSizedAccounts, EmitViaSolLogDataSizedAccounts,
    },
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

/// Real, measured CU cost of both emit mechanisms across a range of payload
/// sizes — answers whether CU scales with payload size, and if so how, for
/// self-CPI vs. `sol_log_data` (`#[event(alloc)]`, `Vec<u8>` field) under
/// solana-program + zero-copy. Every call reuses the same fresh
/// `provider`/blockhash per size so results aren't polluted by replay-
/// avoidance retries.
#[test]
fn payload_size_cu_sweep() {
    let provider = setup();
    let sizes: [u32; 11] = [0, 8, 32, 128, 256, 512, 768, 1024, 1280, 1536, 2048];

    println!("size_bytes,self_cpi_cu,sol_log_data_cu");
    for size in sizes {
        let self_cpi = build_emit_via_self_cpi_sized(
            &provider,
            PROGRAM_ID,
            size,
            EmitViaSelfCpiSizedAccounts { program: PROGRAM_ID },
        )
        .send_and_confirm()
        .unwrap_or_else(|e| panic!("emit_via_self_cpi_sized(size={size}) should succeed: {e:?}"));

        if let ClientBackend::LiteSVM(svm) = &provider.backend {
            svm.lock().unwrap().expire_blockhash();
        }

        let sol_log = build_emit_via_sol_log_data_sized(
            &provider,
            PROGRAM_ID,
            size,
            EmitViaSolLogDataSizedAccounts { program: PROGRAM_ID },
        )
        .send_and_confirm()
        .unwrap_or_else(|e| {
            panic!("emit_via_sol_log_data_sized(size={size}) should succeed: {e:?}")
        });

        if let ClientBackend::LiteSVM(svm) = &provider.backend {
            svm.lock().unwrap().expire_blockhash();
        }

        println!(
            "{size},{},{}",
            self_cpi.compute_units_consumed, sol_log.compute_units_consumed
        );
    }
}
