use vault_client::{
    components::{Vault, UserAccount, fetch_vault, fetch_user_account},
    instructions::{
        build_initialize, build_deposit, build_withdraw,
        InitializeAccounts, DepositAccounts, WithdrawAccounts,
    },
    types::{PROGRAM_ID, FundsDeposited, FundsWithdrawn},
    get_vault_account_pda, get_user_account_pda,
};
use naclac_client::*;

#[test]
fn test_vault_lifecycle() {
    // 1. Setup the provider
    let cluster = "localnet";

    let payer = load_node_wallet().expect("Failed to load local Solana keypair");
    let provider = NaclacProvider::new(cluster, payer);
    println!("\n🔑 Loaded Payer Wallet: {}", provider.payer.address());

    // 2. Setup Program ID & PDA from the generated client SDK
    let program_id = PROGRAM_ID;
    let (vault_pda, _vault_bump) = get_vault_account_pda(&program_id);
    let (user_pda, user_bump) = get_user_account_pda(&program_id, &provider.payer.address());
    let system_program = Address::default();
    println!("   Derived Vault PDA: {}", vault_pda);
    println!("   Derived User Account PDA: {}", user_pda);

    // 3. Load the program binary
    let mut so_path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    so_path.pop(); // programs
    so_path.pop(); // vault root
    so_path.push("target/deploy/vault.so");

    provider
        .add_program(&program_id, so_path.to_str().unwrap())
        .expect("Failed to load vault program binary");
    println!("   📦 Loaded program binary.");

    // 4. Initialize Vault Account (Skip if already initialized)
    let mut already_initialized = false;

    if let Ok(account) = provider.get_account(&vault_pda) {
        if account.data.len() >= 8 {
            already_initialized = true;
            println!("   ℹ️ Vault account already exists. Skipping initialization.");
        }
    }

    if !already_initialized {
        println!("🚀 Sending Initialize transaction...");

        let tx_meta = build_initialize(
            &provider,
            program_id,
            InitializeAccounts {
                payer: provider.payer.address(),
                vault_account: vault_pda,
                system_program,
            },
        )
        .send_and_confirm()
        .unwrap();

        println!(
            "   📊 Initialize Compute Units (CUs): {}",
            tx_meta.compute_units_consumed
        );
        if !tx_meta.logs.is_empty() {
            println!("   📜 Program Logs:");
            for log in &tx_meta.logs {
                println!("      {}", log);
            }
        }

        // Verify account initialized using the generated Vault component
        let vault_state: Vault = fetch_vault(&provider, &vault_pda)
            .expect("Failed to fetch initialized vault account");
        assert_eq!(vault_state.total_deposited, 0);
        assert_eq!(vault_state.authority, provider.payer.address());
        println!("   ✅ Initialized state verified: total_deposited = 0, authority matches payer.");
    }

    // Get current vault and user counts/balances
    let mut start_vault_balance = 0u64;
    let mut start_user_balance = 0u64;

    if let Ok(vault_state) = fetch_vault(&provider, &vault_pda) {
        start_vault_balance = vault_state.total_deposited;
    }
    if let Ok(user_state) = fetch_user_account(&provider, &user_pda) {
        start_user_balance = user_state.balance;
    }

    // 5. Deposit Account
    let deposit_amount = 1_000_000u64; // 0.001 SOL
    println!("💰 Sending Deposit transaction of {} lamports...", deposit_amount);

    let tx_meta = build_deposit(
        &provider,
        program_id,
        deposit_amount,
        user_bump,
        DepositAccounts {
            payer: provider.payer.address(),
            vault_account: vault_pda,
            user_account: user_pda,
            system_program,
        },
    )
    .send_and_confirm()
    .unwrap();

    println!(
        "   📊 Deposit Compute Units (CUs): {}",
        tx_meta.compute_units_consumed
    );
    if !tx_meta.logs.is_empty() {
        println!("   📜 Program Logs:");
        for log in &tx_meta.logs {
            println!("      {}", log);
        }
    }

    // 6. Verify Deposit using the generated Vault/UserAccount component
    let vault_state: Vault = fetch_vault(&provider, &vault_pda)
        .expect("Failed to fetch vault account");
    assert_eq!(vault_state.total_deposited, start_vault_balance + deposit_amount);

    let user_state: UserAccount = fetch_user_account(&provider, &user_pda)
        .expect("Failed to fetch user account");
    assert_eq!(user_state.balance, start_user_balance + deposit_amount);
    println!(
        "   ✅ State verified: vault = {}, user_account = {}.",
        vault_state.total_deposited,
        user_state.balance
    );

    // 7. Verify Deposit Event
    let captured_deposit_events: Vec<FundsDeposited> = tx_meta
        .parse_events_zero_copy()
        .expect("Failed to parse deposit events");
    assert!(!captured_deposit_events.is_empty(), "❌ Deposit event was not captured!");
    let captured_dep_event = &captured_deposit_events[0];
    assert_eq!(captured_dep_event.amount, deposit_amount);
    println!(
        "      🔔 Event Fired! Deposited: {}, Vault Balance: {}",
        captured_dep_event.amount,
        captured_dep_event.total_vault_balance
    );

    // 8. Withdraw Account
    let withdraw_amount = 400_000u64; // 0.0004 SOL
    println!("💸 Sending Withdraw transaction of {} lamports...", withdraw_amount);

    let tx_meta = build_withdraw(
        &provider,
        program_id,
        withdraw_amount,
        WithdrawAccounts {
            user: provider.payer.address(),
            vault_account: vault_pda,
            user_account: user_pda,
            system_program,
        },
    )
    .send_and_confirm()
    .unwrap();

    println!(
        "   📊 Withdraw Compute Units (CUs): {}",
        tx_meta.compute_units_consumed
    );
    if !tx_meta.logs.is_empty() {
        println!("   📜 Program Logs:");
        for log in &tx_meta.logs {
            println!("      {}", log);
        }
    }

    // Verify Withdraw
    let vault_state_after: Vault = fetch_vault(&provider, &vault_pda)
        .expect("Failed to fetch vault account");
    assert_eq!(
        vault_state_after.total_deposited,
        start_vault_balance + deposit_amount - withdraw_amount
    );

    let user_state_after: UserAccount = fetch_user_account(&provider, &user_pda)
        .expect("Failed to fetch user account");
    assert_eq!(
        user_state_after.balance,
        start_user_balance + deposit_amount - withdraw_amount
    );
    println!(
        "   ✅ State verified: vault = {}, user_account = {}.",
        vault_state_after.total_deposited,
        user_state_after.balance
    );

    // 9. Verify Withdraw Event
    let captured_withdraw_events: Vec<FundsWithdrawn> = tx_meta
        .parse_events_zero_copy()
        .expect("Failed to parse withdraw events");
    assert!(!captured_withdraw_events.is_empty(), "❌ Withdraw event was not captured!");
    let captured_with_event = &captured_withdraw_events[0];
    assert_eq!(captured_with_event.amount, withdraw_amount);
    println!(
        "      🔔 Event Fired! Withdrawn: {}, Vault Balance: {}",
        captured_with_event.amount,
        captured_with_event.total_vault_balance
    );

    // 10. Excessive Withdraw (should fail)
    let excessive_amount = 10_000_000u64;
    println!("⚠️ Attempting excessive withdrawal of {} lamports...", excessive_amount);
    let res = build_withdraw(
        &provider,
        program_id,
        excessive_amount,
        WithdrawAccounts {
            user: provider.payer.address(),
            vault_account: vault_pda,
            user_account: user_pda,
            system_program,
        },
    )
    .send_and_confirm();
    
    assert!(res.is_err());
    let err = res.err().unwrap();
    match err {
        NaclacClientError::TransactionFailed { instruction_err, .. } => {
            // VaultError::InsufficientFunds is index 1, offset by 6000 = 6001
            assert_eq!(instruction_err, InstructionError::Custom(6001));
            println!("   ✅ Correctly failed to withdraw excessive amount with custom error 6001.");
        }
        _ => panic!("Expected TransactionFailed error, got {:?}", err),
    }

    println!("\n✨ Vault integration test completed successfully!\n");
}
