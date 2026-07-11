use token_vault_zc_client::{
    components::{Vault, UserAccount, fetch_vault, fetch_user_account},
    instructions::{
        build_initialize, build_deposit, build_withdraw,
        InitializeAccounts, DepositAccounts, WithdrawAccounts,
    },
    types::{PROGRAM_ID, TokenDeposited, TokenWithdrawn},
    get_vault_account_pda, get_user_account_pda,
};
use naclac_client::*;
use naclac_client::utils::{
    create_mint_with_program, create_ata_with_program, mint_to_with_program,
};

#[allow(dead_code)]
pub fn run_lifecycle_test(token_program_id: Address) {
    // Determine a friendly name for logs
    let name = if token_program_id == naclac_client::utils::TOKEN_PROGRAM_ID {
        "SPL Token"
    } else {
        "Token-2022"
    };

    // 1. Setup the provider
    let cluster = "litesvm";

    let payer = load_node_wallet().expect("Failed to load local Solana keypair");
    let provider = NaclacProvider::new(cluster, payer);
    println!("\n🔑 [{}] Loaded Payer Wallet: {}", name, provider.payer.address());

    // 2. Setup Mint
    let mint_signer = Keypair::new();
    let mint = mint_signer.address();
    println!("🪙 [{}] Creating Mint: {}", name, mint);
    create_mint_with_program(&provider, &mint_signer, &provider.payer.address(), 9, &token_program_id)
        .expect("Failed to create mint");

    // 3. Setup Program ID & PDA from the generated client SDK
    let program_id = PROGRAM_ID;
    let vault_id = 42u64;
    let (vault_pda, vault_bump) = get_vault_account_pda(&program_id, vault_id);
    let (user_pda, user_bump) = get_user_account_pda(&program_id, vault_id, &provider.payer.address(), &mint);
    let system_program = Address::default();
    println!("   [{}] Derived Vault PDA: {}", name, vault_pda);
    println!("   [{}] Derived User Account PDA: {}", name, user_pda);

    // 4. Setup Associated Token Accounts
    println!("   [{}] Creating Associated Token Accounts...", name);
    let user_token_account = create_ata_with_program(&provider, &mint, &provider.payer.address(), &token_program_id)
        .expect("Failed to create user ATA");
    let vault_token_account = create_ata_with_program(&provider, &mint, &vault_pda, &token_program_id)
        .expect("Failed to create vault ATA");
    println!("      User ATA: {}", user_token_account);
    println!("      Vault ATA: {}", vault_token_account);

    // Mint initial tokens to user ATA
    let initial_mint_amount = 1_000_000_000u64; // 1,000,000,000 tokens
    println!("      [{}] Minting {} tokens to User ATA...", name, initial_mint_amount);
    mint_to_with_program(&provider, &mint, &user_token_account, &provider.payer, initial_mint_amount, &token_program_id)
        .expect("Failed to mint tokens to user");

    // 5. Load the program binary
    let mut so_path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    so_path.pop(); // programs
    so_path.pop(); // token-vault root
    so_path.push("target/deploy/token_vault_zc.so");

    provider
        .add_program(&program_id, so_path.to_str().unwrap())
        .expect("Failed to load vault program binary");
    println!("   [{}] 📦 Loaded program binary.", name);

    // 6. Initialize Vault Account (Skip if already initialized)
    let fetcher = AccountFetcher::new(&provider);
    let mut already_initialized = false;

    if let Ok(account) = provider.get_account(&vault_pda) {
        if account.data.len() >= 8 {
            already_initialized = true;
            println!("   ℹ️ [{}] Vault account already exists. Skipping initialization.", name);
        }
    }

    if !already_initialized {
        println!("🚀 [{}] Sending Initialize transaction...", name);

        let tx_meta = build_initialize(
            &provider,
            program_id,
            vault_id,
            vault_bump,
            InitializeAccounts {
                payer: provider.payer.address(),
                vault_account: vault_pda,
                system_program,
            },
        )
        .send_and_confirm()
        .unwrap();

        println!(
            "   📊 [{}] Initialize Compute Units (CUs): {}",
            name,
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
        assert_eq!(vault_state.authority, provider.payer.address());
        assert_eq!(vault_state.vault_id, vault_id);
        assert_eq!(vault_state.bump, vault_bump);
        println!("   ✅ [{}] Initialized state verified: authority matches payer.", name);
    }

    // Get starting balance of user account PDA
    let mut start_user_account_balance = 0u64;
    if let Ok(user_state) = fetch_user_account(&provider, &user_pda) {
        start_user_account_balance = user_state.balance;
    }

    // 7. Deposit Tokens
    let deposit_amount = 500_000_000u64; // 500,000,000 tokens
    println!("💰 [{}] Sending FIRST Deposit transaction of {} tokens...", name, deposit_amount);

    let tx_meta = build_deposit(
        &provider,
        program_id,
        vault_id,
        deposit_amount,
        user_bump,
        DepositAccounts {
            payer: provider.payer.address(),
            mint,
            vault_account: vault_pda,
            vault_token_account,
            user_token_account,
            user_account: user_pda,
            token_program: token_program_id,
            system_program,
        },
    )
    .send_and_confirm()
    .unwrap();

    println!(
        "   📊 [{}] First Deposit Compute Units (CUs): {}",
        name,
        tx_meta.compute_units_consumed
    );
    if !tx_meta.logs.is_empty() {
        println!("   📜 Program Logs:");
        for log in &tx_meta.logs {
            println!("      {}", log);
        }
    }

    // 7.5. Deposit Tokens Again (to confirm init_if_needed behavior / CU drop)
    let second_deposit_amount = 100_000_000u64; // 100,000,000 tokens
    println!("💰 [{}] Sending SECOND Deposit transaction of {} tokens...", name, second_deposit_amount);

    let tx_meta_second = build_deposit(
        &provider,
        program_id,
        vault_id,
        second_deposit_amount,
        user_bump,
        DepositAccounts {
            payer: provider.payer.address(),
            mint,
            vault_account: vault_pda,
            vault_token_account,
            user_token_account,
            user_account: user_pda,
            token_program: token_program_id,
            system_program,
        },
    )
    .send_and_confirm()
    .unwrap();

    println!(
        "   📊 [{}] Second Deposit Compute Units (CUs): {}",
        name,
        tx_meta_second.compute_units_consumed
    );
    if !tx_meta_second.logs.is_empty() {
        println!("   📜 Second Program Logs:");
        for log in &tx_meta_second.logs {
            println!("      {}", log);
        }
    }

    // 8. Verify Deposit using the generated components and token accounts
    let user_state: UserAccount = fetch_user_account(&provider, &user_pda)
        .expect("Failed to fetch user account");
    assert_eq!(user_state.balance, start_user_account_balance + deposit_amount + second_deposit_amount);
    assert_eq!(user_state.owner, provider.payer.address());

    let user_token_data: TokenAccount = fetcher
        .fetch(&user_token_account)
        .expect("Failed to fetch user token account");
    assert_eq!(user_token_data.amount(), initial_mint_amount - deposit_amount - second_deposit_amount);

    let vault_token_data: TokenAccount = fetcher
        .fetch(&vault_token_account)
        .expect("Failed to fetch vault token account");
    assert_eq!(vault_token_data.amount(), deposit_amount + second_deposit_amount);

    println!(
        "   ✅ [{}] State verified: user_account = {}, User ATA = {}, Vault ATA = {}.",
        name,
        user_state.balance,
        user_token_data.amount(),
        vault_token_data.amount()
    );

    // 9. Verify Deposit Event (First Deposit)
    let captured_deposit_events: Vec<TokenDeposited> = tx_meta
        .parse_events_zero_copy()
        .expect("Failed to parse deposit events");
    assert!(!captured_deposit_events.is_empty(), "❌ Deposit event was not captured!");
    let captured_dep_event = &captured_deposit_events[0];
    assert_eq!(captured_dep_event.amount, deposit_amount);
    assert_eq!(captured_dep_event.user, provider.payer.address());
    println!(
        "      🔔 [{}] Event Fired! Deposited: {}, Vault Balance: {}",
        name,
        captured_dep_event.amount,
        captured_dep_event.total_vault_balance
    );

    // 10. Withdraw Tokens
    let withdraw_amount = 250_000_000u64; // 250,000,000 tokens
    println!("💸 [{}] Sending Withdraw transaction of {} tokens...", name, withdraw_amount);

    let tx_meta_withdraw = build_withdraw(
        &provider,
        program_id,
        vault_id,
        withdraw_amount,
        WithdrawAccounts {
            payer: provider.payer.address(),
            mint,
            vault_account: vault_pda,
            vault_token_account,
            user_token_account,
            user_account: user_pda,
            token_program: token_program_id,
        },
    )
    .send_and_confirm()
    .unwrap();

    println!(
        "   📊 [{}] Withdraw Compute Units (CUs): {}",
        name,
        tx_meta_withdraw.compute_units_consumed
    );
    if !tx_meta_withdraw.logs.is_empty() {
        println!("   📜 Program Logs:");
        for log in &tx_meta_withdraw.logs {
            println!("      {}", log);
        }
    }

    // Verify Withdraw
    let user_state_after: UserAccount = fetch_user_account(&provider, &user_pda)
        .expect("Failed to fetch user account");
    assert_eq!(
        user_state_after.balance,
        start_user_account_balance + deposit_amount + second_deposit_amount - withdraw_amount
    );

    let user_token_data_after: TokenAccount = fetcher
        .fetch(&user_token_account)
        .expect("Failed to fetch user token account");
    assert_eq!(
        user_token_data_after.amount(),
        initial_mint_amount - deposit_amount - second_deposit_amount + withdraw_amount
    );

    let vault_token_data_after: TokenAccount = fetcher
        .fetch(&vault_token_account)
        .expect("Failed to fetch vault token account");
    assert_eq!(
        vault_token_data_after.amount(),
        deposit_amount + second_deposit_amount - withdraw_amount
    );

    println!(
        "   ✅ [{}] State verified: user_account = {}, User ATA = {}, Vault ATA = {}.",
        name,
        user_state_after.balance,
        user_token_data_after.amount(),
        vault_token_data_after.amount()
    );

    // 11. Verify Withdraw Event
    let captured_withdraw_events: Vec<TokenWithdrawn> = tx_meta_withdraw
        .parse_events_zero_copy()
        .expect("Failed to parse withdraw events");
    assert!(!captured_withdraw_events.is_empty(), "❌ Withdraw event was not captured!");
    let captured_with_event = &captured_withdraw_events[0];
    assert_eq!(captured_with_event.amount, withdraw_amount);
    assert_eq!(captured_with_event.user, provider.payer.address());
    println!(
        "      🔔 [{}] Event Fired! Withdrawn: {}, Vault Balance: {}",
        name,
        captured_with_event.amount,
        captured_with_event.total_vault_balance
    );

    // 12. Excessive Withdraw (should fail)
    let excessive_amount = 1_000_000_000u64; // 1,000,000,000 tokens
    println!("⚠️ [{}] Attempting excessive withdrawal of {} tokens...", name, excessive_amount);
    let res = build_withdraw(
        &provider,
        program_id,
        vault_id,
        excessive_amount,
        WithdrawAccounts {
            payer: provider.payer.address(),
            mint,
            vault_account: vault_pda,
            vault_token_account,
            user_token_account,
            user_account: user_pda,
            token_program: token_program_id,
        },
    )
    .send_and_confirm();
    
    assert!(res.is_err());
    let err = res.err().unwrap();
    match err {
        NaclacClientError::TransactionFailed { instruction_err, .. } => {
            assert_eq!(instruction_err, InstructionError::Custom(6001));
            println!("   ✅ [{}] Correctly failed to withdraw excessive amount with custom error 6001.", name);
        }
        _ => panic!("Expected TransactionFailed error, got {:?}", err),
    }

    println!("\n✨ [{}] Token Vault integration test completed successfully!\n", name);
}
