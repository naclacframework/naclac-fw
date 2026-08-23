use naclac_client::*;
use counter_zc_client::{
    instructions::{
        build_initialize, build_increment,
        InitializeAccounts, IncrementAccounts,
    },
    components::{Counter, fetch_counter},
    types::{PROGRAM_ID, CounterIncremented},
    get_counter_account_pda,
};

#[test]
fn test_counter_integration() {
    // 1. Setup the provider
    // Change this to "devnet" or an RPC URL to test against a live network!
    let cluster = "litesvm";

    let payer = load_node_wallet().expect("Failed to load local Solana keypair");
    let provider = NaclacProvider::new(cluster, payer);
    println!("\n🔑 Loaded Payer Wallet: {}", provider.payer.address());

    // 2. Setup Program ID & PDA from the generated client SDK
    let program_id = PROGRAM_ID;
    let (counter_pda, _bump) = get_counter_account_pda(&program_id);
    println!("   Derived PDA: {}", counter_pda);

    // 3. Load the program binary
    let mut workspace_root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    workspace_root.pop(); // programs
    workspace_root.pop(); // pump-amm workspace root
    let so_path = resolve_cargo_target_dir(&workspace_root).join("deploy/counter_zc.so");

    provider.add_program(&program_id, so_path.to_str().unwrap())
        .expect("Failed to load counter_zc program binary");
    println!("   📦 Loaded program binary.");

    // 4. Initialize Account (Skip if already initialized)
    let mut already_initialized = false;
    let mut start_count = 0u64;

    if let Ok(account) = provider.get_account(&counter_pda) {
        if account.data.len() >= 16 {
            let data = &account.data[8..];
            start_count = u64::from_le_bytes(data[0..8].try_into().unwrap());
            already_initialized = true;
            println!("   ℹ️ Counter account already exists on chain (Current Count: {}). Skipping initialization.", start_count);
        }
    }

    if !already_initialized {
        println!("🚀 Sending Initialize transaction...");

        let tx_meta = build_initialize(
            &provider,
            program_id,
            InitializeAccounts {
                payer: provider.payer.address(),
                counter_account: counter_pda,
                system_program: Address::default(),
            },
        )
        .send_and_confirm()
        .unwrap();

        println!("   📊 Initialize Compute Units (CUs): {}", tx_meta.compute_units_consumed);
        if !tx_meta.logs.is_empty() {
            println!("   📜 Program Logs:");
            for log in &tx_meta.logs {
                println!("      {}", log);
            }
        }

        // Verify account initialized using the generated Counter component
        let counter_state: Counter = fetch_counter(&provider, &counter_pda)
            .expect("Failed to fetch initialized counter account");
        assert_eq!(counter_state.count, 0);
        assert_eq!(counter_state.authority, provider.payer.address());
        println!("   ✅ Initialized state verified: count = 0, authority matches payer.");
    }

    // 5. Increment Account
    println!("📈 Sending Increment transaction...");

    let tx_meta = build_increment(
        &provider,
        program_id,
        IncrementAccounts {
            authority: provider.payer.address(),
            counter_account: counter_pda,
        },
    )
    .send_and_confirm()
    .unwrap();

    println!("   📊 Increment Compute Units (CUs): {}", tx_meta.compute_units_consumed);
    if !tx_meta.logs.is_empty() {
        println!("   📜 Program Logs:");
        for log in &tx_meta.logs {
            println!("      {}", log);
        }
    }

    // 6. Verify Increment using the generated Counter component
    let counter_state_after: Counter = fetch_counter(&provider, &counter_pda)
        .expect("Failed to fetch incremented counter account");
    assert_eq!(counter_state_after.count, start_count + 1);
    println!("   ✅ State verified: count = {}.", counter_state_after.count);

    // 7. Verify Event using the generated CounterIncremented event type (Borsh)
    let captured_events: Vec<CounterIncremented> = tx_meta.parse_events_zero_copy()
        .expect("Failed to parse events");
    assert!(!captured_events.is_empty(), "❌ Event was not captured!");
    let captured_event = &captured_events[0];
    assert_eq!(captured_event.new_count, start_count + 1);
    println!("      🔔 Event Fired! New Count: {}", captured_event.new_count);

    println!("\n✨ Integration test completed successfully!\n");
}
