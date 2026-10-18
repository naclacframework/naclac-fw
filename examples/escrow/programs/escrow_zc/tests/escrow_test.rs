use {
    escrow_zc_client::{
        components::{fetch_escrow_state, EscrowState},
        get_escrow_state_pda,
        instructions::{
            build_cancel, build_make, build_take,
            CancelAccounts, MakeAccounts, TakeAccounts,
        },
        types::{EscrowCancelled, EscrowCreated, EscrowExchanged, PROGRAM_ID},
    },
    naclac_client::{
        *,
        utils::{
            create_ata_with_program, create_mint_with_program, load_node_wallet,
            mint_to_with_program, transfer_sol, TOKEN_PROGRAM_ID,
        },
    },
};

#[test]
fn test_escrow_lifecycle() {
    let name = "SPL Token";
    println!("\n🚀 [{}] Starting Escrow Lifecycle Test...", name);

    // 1. Setup the provider
    let cluster = "litesvm";
    let payer = load_node_wallet().expect("Failed to load local Solana keypair");
    let provider = NaclacProvider::new(cluster, payer).expect("Failed to construct NaclacProvider");
    let token_program_id = TOKEN_PROGRAM_ID;

    let maker_address = provider.payer.address();
    let taker = Keypair::new();
    let taker_address = taker.address();
    transfer_sol(&provider, &taker_address, 1_000_000_000).expect("Failed to fund taker");

    // 2. Setup Mint A and Mint B
    let mint_a_signer = Keypair::new();
    let mint_a = mint_a_signer.address();
    create_mint_with_program(&provider, &mint_a_signer, &provider.payer.address(), 9, &token_program_id)
        .expect("Failed to create mint A");

    let mint_b_signer = Keypair::new();
    let mint_b = mint_b_signer.address();
    create_mint_with_program(&provider, &mint_b_signer, &provider.payer.address(), 9, &token_program_id)
        .expect("Failed to create mint B");

    // 3. Setup Associated Token Accounts
    let maker_token_account_a = create_ata_with_program(&provider, &mint_a, &maker_address, &token_program_id)
        .expect("Failed to create maker ATA A");
    let maker_token_account_b = create_ata_with_program(&provider, &mint_b, &maker_address, &token_program_id)
        .expect("Failed to create maker ATA B");
    let taker_token_account_a = create_ata_with_program(&provider, &mint_a, &taker_address, &token_program_id)
        .expect("Failed to create taker ATA A");
    let taker_token_account_b = create_ata_with_program(&provider, &mint_b, &taker_address, &token_program_id)
        .expect("Failed to create taker ATA B");

    // Mint initial tokens
    let initial_maker_a = 1_000_000_000u64;
    let initial_taker_b = 2_000_000_000u64;
    mint_to_with_program(&provider, &mint_a, &maker_token_account_a, &provider.payer, initial_maker_a, &token_program_id)
        .expect("Failed to mint to maker ATA A");
    mint_to_with_program(&provider, &mint_b, &taker_token_account_b, &provider.payer, initial_taker_b, &token_program_id)
        .expect("Failed to mint to taker ATA B");

    let fetcher = AccountFetcher::new(&provider);

    // =========================================================================
    // FLOW 1: Make & Cancel
    // =========================================================================
    let seed_cancel = 789012u64;
    let (escrow_pda_cancel, _escrow_bump_cancel) = get_escrow_state_pda(&PROGRAM_ID, &maker_address, seed_cancel);

    let mut already_initialized_cancel = false;
    if let Ok(account) = provider.get_account(&escrow_pda_cancel) {
        if account.data.len() >= 8 {
            already_initialized_cancel = true;
            println!("   ℹ️ Escrow account for cancel already exists. Skipping Make.");
        }
    }

    let vault_token_account_cancel = create_ata_with_program(&provider, &mint_a, &escrow_pda_cancel, &token_program_id)
        .expect("Failed to create vault ATA for cancel");

    let amount_a_cancel = 400_000_000u64;
    let amount_b_cancel = 800_000_000u64;

    if !already_initialized_cancel {
        println!("🤝 Sending MAKE transaction for Cancel Flow (A = {}, B = {})...", amount_a_cancel, amount_b_cancel);

        let tx_meta = build_make(
            &provider,
            PROGRAM_ID,
            seed_cancel,
            amount_a_cancel,
            amount_b_cancel,
            MakeAccounts {
                maker: maker_address,
                mint_a,
                mint_b,
                escrow_state: escrow_pda_cancel,
                vault_token_account: vault_token_account_cancel,
                maker_token_account_a,
                token_program: token_program_id,
                system_program: Address::default(),
            },
        )
        .send_and_confirm()
        .unwrap();

        println!("   📊 Make Compute Units (Cancel Flow): {}", tx_meta.compute_units_consumed);
    }

    // Verify on-chain escrow state and vault balance
    let escrow_state: EscrowState = fetch_escrow_state(&provider, &escrow_pda_cancel)
        .expect("Failed to fetch escrow state");
    let escrow_amount_a = escrow_state.amount_a;
    assert_eq!(escrow_amount_a, amount_a_cancel);

    let vault_balance: TokenAccount = fetcher
        .fetch(&vault_token_account_cancel)
        .expect("Failed to fetch vault token account");
    assert_eq!(vault_balance.amount(), amount_a_cancel);

    // Cancel Escrow
    println!("🛑 Sending CANCEL transaction...");

    let tx_meta_cancel = build_cancel(
        &provider,
        PROGRAM_ID,
        seed_cancel,
        CancelAccounts {
            maker: maker_address,
            mint_a,
            escrow_state: escrow_pda_cancel,
            vault_token_account: vault_token_account_cancel,
            maker_token_account_a,
            token_program: token_program_id,
            system_program: Address::default(),
        },
    )
    .send_and_confirm()
    .unwrap();

    println!("   📊 Cancel Compute Units: {}", tx_meta_cancel.compute_units_consumed);

    // Parse and verify EscrowCancelled event
    let cancel_events: Vec<EscrowCancelled> = tx_meta_cancel
        .parse_events_zero_copy()
        .expect("Failed to parse cancel events");
    assert!(!cancel_events.is_empty(), "EscrowCancelled event not emitted");
    assert_eq!(cancel_events[0].maker, maker_address);
    let cancel_amount_a = cancel_events[0].amount_a;
    assert_eq!(cancel_amount_a, amount_a_cancel);

    // Verify balances after CANCEL
    let maker_a_balance: TokenAccount = fetcher
        .fetch(&maker_token_account_a)
        .expect("Failed to fetch maker token account A");
    assert_eq!(maker_a_balance.amount(), initial_maker_a);

    // Verify accounts closed
    assert!(provider.get_account(&escrow_pda_cancel).is_err(), "Escrow state PDA not closed");
    assert!(provider.get_account(&vault_token_account_cancel).is_err(), "Vault token account not closed");
    println!("   ✅ Cancel Flow completed and verified successfully.");

    // =========================================================================
    // FLOW 2: Make & Take
    // =========================================================================
    let seed_take = 123456u64;
    let (escrow_pda_take, _escrow_bump_take) = get_escrow_state_pda(&PROGRAM_ID, &maker_address, seed_take);

    let mut already_initialized_take = false;
    if let Ok(account) = provider.get_account(&escrow_pda_take) {
        if account.data.len() >= 8 {
            already_initialized_take = true;
            println!("   ℹ️ Escrow account for take already exists. Skipping Make.");
        }
    }

    let vault_token_account_take = create_ata_with_program(&provider, &mint_a, &escrow_pda_take, &token_program_id)
        .expect("Failed to create vault ATA for take");

    let amount_a_take = 500_000_000u64;
    let amount_b_take = 1_000_000_000u64;

    if !already_initialized_take {
        println!("🤝 Sending MAKE transaction for Take Flow (A = {}, B = {})...", amount_a_take, amount_b_take);

        let tx_meta = build_make(
            &provider,
            PROGRAM_ID,
            seed_take,
            amount_a_take,
            amount_b_take,
            MakeAccounts {
                maker: maker_address,
                mint_a,
                mint_b,
                escrow_state: escrow_pda_take,
                vault_token_account: vault_token_account_take,
                maker_token_account_a,
                token_program: token_program_id,
                system_program: Address::default(),
            },
        )
        .send_and_confirm()
        .unwrap();

        println!("   📊 Make Compute Units (Take Flow): {}", tx_meta.compute_units_consumed);

        // Parse and verify EscrowCreated event
        let captured_events: Vec<EscrowCreated> = tx_meta
            .parse_events_zero_copy()
            .expect("Failed to parse events");
        assert!(!captured_events.is_empty(), "EscrowCreated event not emitted");
        assert_eq!(captured_events[0].maker, maker_address);
        let captured_amount_a = captured_events[0].amount_a;
        let captured_amount_b = captured_events[0].amount_b;
        assert_eq!(captured_amount_a, amount_a_take);
        assert_eq!(captured_amount_b, amount_b_take);
    }

    // Fetch and verify EscrowState PDA state
    let escrow_state_take: EscrowState = fetch_escrow_state(&provider, &escrow_pda_take)
        .expect("Failed to fetch escrow state PDA");
    assert_eq!(escrow_state_take.maker, maker_address);
    assert_eq!(escrow_state_take.mint_a, mint_a);
    assert_eq!(escrow_state_take.mint_b, mint_b);
    let take_state_amount_a = escrow_state_take.amount_a;
    let take_state_amount_b = escrow_state_take.amount_b;
    assert_eq!(take_state_amount_a, amount_a_take);
    assert_eq!(take_state_amount_b, amount_b_take);

    // Verify vault balance
    let vault_balance_take: TokenAccount = fetcher
        .fetch(&vault_token_account_take)
        .expect("Failed to fetch vault token account");
    assert_eq!(vault_balance_take.amount(), amount_a_take);

    // Take Escrow
    println!("🎬 Sending TAKE transaction...");

    let tx_meta_take = build_take(
        &provider,
        PROGRAM_ID,
        seed_take,
        TakeAccounts {
            taker: taker_address,
            maker: maker_address,
            mint_a,
            mint_b,
            escrow_state: escrow_pda_take,
            vault_token_account: vault_token_account_take,
            taker_token_account_a,
            taker_token_account_b,
            maker_token_account_b,
            token_program: token_program_id,
            system_program: Address::default(),
        },
    )
    .signer(&taker)
    .send_and_confirm()
    .unwrap();

    println!("   📊 Take Compute Units: {}", tx_meta_take.compute_units_consumed);

    // Parse and verify EscrowExchanged event
    let take_events: Vec<EscrowExchanged> = tx_meta_take
        .parse_events_zero_copy()
        .expect("Failed to parse take events");
    assert!(!take_events.is_empty(), "EscrowExchanged event not emitted");
    assert_eq!(take_events[0].taker, taker_address);
    let take_amount_a = take_events[0].amount_a;
    let take_amount_b = take_events[0].amount_b;
    assert_eq!(take_amount_a, amount_a_take);
    assert_eq!(take_amount_b, amount_b_take);

    // Verify balances after TAKE
    let maker_b_balance: TokenAccount = fetcher
        .fetch(&maker_token_account_b)
        .expect("Failed to fetch maker token account B");
    assert_eq!(maker_b_balance.amount(), amount_b_take);

    let taker_b_balance: TokenAccount = fetcher
        .fetch(&taker_token_account_b)
        .expect("Failed to fetch taker token account B");
    assert_eq!(taker_b_balance.amount(), initial_taker_b - amount_b_take);

    let taker_a_balance: TokenAccount = fetcher
        .fetch(&taker_token_account_a)
        .expect("Failed to fetch taker token account A");
    assert_eq!(taker_a_balance.amount(), amount_a_take);

    let maker_a_balance: TokenAccount = fetcher
        .fetch(&maker_token_account_a)
        .expect("Failed to fetch maker token account A");
    assert_eq!(maker_a_balance.amount(), initial_maker_a - amount_a_take);

    // Verify accounts closed
    assert!(provider.get_account(&escrow_pda_take).is_err(), "Escrow state PDA not closed");
    assert!(provider.get_account(&vault_token_account_take).is_err(), "Vault token account not closed");

    println!("   ✅ Take Flow completed and verified successfully.");
    println!("\n✨ Escrow integration test completed successfully!\n");
}
