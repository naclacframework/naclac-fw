use {
    counter_zc_client::{
        components::{fetch_counter, Counter},
        get_counter_account_pda,
        instructions::{
            build_decrement, build_increment, build_initialize,
            DecrementAccounts, IncrementAccounts, InitializeAccounts,
        },
        types::{CounterDecremented, CounterIncremented, PROGRAM_ID},
    },
    naclac_client::*,
};
                       
#[test]
fn test_counter_integration() {
    // 1. Setup the provider
    // Change this to "devnet" or an RPC URL to test against a live network!
    let cluster = "litesvm";

    let payer = load_node_wallet().expect("Failed to load local Solana keypair");
    let provider = NaclacProvider::new(cluster, payer).expect("Failed to construct NaclacProvider");
    println!("\n🔑 Loaded Payer Wallet: {}", provider.payer.address());

    // 2. Setup Program ID & PDA from the generated client SDK
    let program_id = PROGRAM_ID;
    let (counter_pda, _bump) = get_counter_account_pda(&program_id);
    println!("   Derived PDA: {}", counter_pda);

    // 3. Initialize Account (Skip if already initialized)
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

        build_initialize(
            &provider,
            program_id,
            InitializeAccounts {
                payer: provider.payer.address(),
                counter_account: counter_pda,
                system_program: Address::default(),
            },
        )
        .log()
        .send_and_confirm()
        .unwrap();

        // Verify account initialized using the generated Counter component
        let counter_state: Counter = fetch_counter(&provider, &counter_pda)
            .expect("Failed to fetch initialized counter account");
        let count = counter_state.count;
        assert_eq!(count, 0);
        assert_eq!(counter_state.authority, provider.payer.address());
        println!("   ✅ Initialized state verified: count = 0, authority matches payer.");
    }

    // 4. Increment Account
    println!("📈 Sending Increment transaction...");

    let tx_meta = build_increment(
        &provider,
        program_id,
        IncrementAccounts {
            authority: provider.payer.address(),
            counter_account: counter_pda,
        },
    )
    .log()
    .send_and_confirm()
    .unwrap();

    // 5. Verify Increment using the generated Counter component
    let counter_state_after: Counter = fetch_counter(&provider, &counter_pda)
        .expect("Failed to fetch incremented counter account");
    let count_after = counter_state_after.count;
    assert_eq!(count_after, start_count + 1);
    println!("   ✅ State verified: count = {}.", count_after);

    // 6. Verify Event using the generated CounterIncremented event type (zero-copy / POD)
    let captured_events: Vec<CounterIncremented> = tx_meta.parse_events_zero_copy()
        .expect("Failed to parse events");
    assert!(!captured_events.is_empty(), "❌ Event was not captured!");
    let captured_event = &captured_events[0];
    let new_count = captured_event.new_count;
    assert_eq!(new_count, start_count + 1);
    println!("      🔔 Event Fired! New Count: {}", new_count);

    // 7. Decrement Account
    println!("📉 Sending Decrement transaction...");

    let tx_meta_dec = build_decrement(
        &provider,
        program_id,
        DecrementAccounts {
            authority: provider.payer.address(),
            counter_account: counter_pda,
        },
    )
    .log()
    .send_and_confirm()
    .unwrap();

    // 8. Verify Decrement using the generated Counter component
    let counter_state_dec: Counter = fetch_counter(&provider, &counter_pda)
        .expect("Failed to fetch decremented counter account");
    let count_dec = counter_state_dec.count;
    assert_eq!(count_dec, count_after - 1);
    println!("   ✅ State verified: count = {}.", count_dec);

    // 9. Verify Event using the generated CounterDecremented event type (zero-copy / POD)
    let dec_events: Vec<CounterDecremented> = tx_meta_dec.parse_events_zero_copy()
        .expect("Failed to parse events");
    assert!(!dec_events.is_empty(), "❌ Event was not captured!");
    let dec_event = &dec_events[0];
    let new_count_dec = dec_event.new_count;
    assert_eq!(new_count_dec, count_after - 1);
    println!("      🔔 Event Fired! New Count: {}", new_count_dec);

    println!("\n✨ Integration test completed successfully!\n");
}
