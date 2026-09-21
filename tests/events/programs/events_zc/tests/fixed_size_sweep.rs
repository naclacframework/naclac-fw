use naclac_client::*;
use events_zc_client::{
    instructions::{
        build_emit_fixed_128, build_emit_fixed_2048, build_emit_fixed_512, build_emit_fixed_8,
        build_emit_via_self_cpi_fixed_128, build_emit_via_self_cpi_fixed_2048,
        build_emit_via_self_cpi_fixed_512, build_emit_via_self_cpi_fixed_8,
        EmitFixed128Accounts, EmitFixed2048Accounts, EmitFixed512Accounts, EmitFixed8Accounts,
        EmitViaSelfCpiFixed128Accounts, EmitViaSelfCpiFixed2048Accounts,
        EmitViaSelfCpiFixed512Accounts, EmitViaSelfCpiFixed8Accounts,
    },
    types::PROGRAM_ID,
};

fn setup() -> NaclacProvider {
    let payer = load_node_wallet().expect("Failed to load local Solana keypair");
    NaclacProvider::new("litesvm", payer).expect("Failed to construct NaclacProvider")
}

fn expire(provider: &NaclacProvider) {
    if let ClientBackend::LiteSVM(svm) = &provider.backend {
        svm.lock().unwrap().expire_blockhash();
    }
}

/// Real, measured CU cost of the fixed-size (`bytemuck::bytes_of`, no
/// allocation) `sol_log_data` emit path vs. a length-prefix-free self-CPI,
/// across the same sizes used in the dynamic (`#[event(alloc)]`) sweep â€” so
/// the two mechanisms can be compared directly under solana-program +
/// zero-copy.
#[test]
fn fixed_size_cu_sweep() {
    let provider = setup();

    let fixed_8 = build_emit_fixed_8(&provider, PROGRAM_ID, EmitFixed8Accounts { program: PROGRAM_ID })
        .send_and_confirm()
        .expect("emit_fixed_8 should succeed");
    expire(&provider);

    let self_cpi_8 = build_emit_via_self_cpi_fixed_8(
        &provider,
        PROGRAM_ID,
        EmitViaSelfCpiFixed8Accounts { program: PROGRAM_ID },
    )
    .send_and_confirm()
    .expect("emit_via_self_cpi_fixed_8 should succeed");
    expire(&provider);

    let fixed_128 = build_emit_fixed_128(&provider, PROGRAM_ID, EmitFixed128Accounts { program: PROGRAM_ID })
        .send_and_confirm()
        .expect("emit_fixed_128 should succeed");
    expire(&provider);

    let self_cpi_128 = build_emit_via_self_cpi_fixed_128(
        &provider,
        PROGRAM_ID,
        EmitViaSelfCpiFixed128Accounts { program: PROGRAM_ID },
    )
    .send_and_confirm()
    .expect("emit_via_self_cpi_fixed_128 should succeed");
    expire(&provider);

    let fixed_512 = build_emit_fixed_512(&provider, PROGRAM_ID, EmitFixed512Accounts { program: PROGRAM_ID })
        .send_and_confirm()
        .expect("emit_fixed_512 should succeed");
    expire(&provider);

    let self_cpi_512 = build_emit_via_self_cpi_fixed_512(
        &provider,
        PROGRAM_ID,
        EmitViaSelfCpiFixed512Accounts { program: PROGRAM_ID },
    )
    .send_and_confirm()
    .expect("emit_via_self_cpi_fixed_512 should succeed");
    expire(&provider);

    let fixed_2048 = build_emit_fixed_2048(&provider, PROGRAM_ID, EmitFixed2048Accounts { program: PROGRAM_ID })
        .send_and_confirm()
        .expect("emit_fixed_2048 should succeed");
    expire(&provider);

    let self_cpi_2048 = build_emit_via_self_cpi_fixed_2048(
        &provider,
        PROGRAM_ID,
        EmitViaSelfCpiFixed2048Accounts { program: PROGRAM_ID },
    )
    .send_and_confirm()
    .expect("emit_via_self_cpi_fixed_2048 should succeed");

    println!("size_bytes,fixed_sol_log_data_cu,fixed_self_cpi_cu");
    println!("8,{},{}", fixed_8.compute_units_consumed, self_cpi_8.compute_units_consumed);
    println!("128,{},{}", fixed_128.compute_units_consumed, self_cpi_128.compute_units_consumed);
    println!("512,{},{}", fixed_512.compute_units_consumed, self_cpi_512.compute_units_consumed);
    println!("2048,{},{}", fixed_2048.compute_units_consumed, self_cpi_2048.compute_units_consumed);
}
