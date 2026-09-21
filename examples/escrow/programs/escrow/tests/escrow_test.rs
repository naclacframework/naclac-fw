use escrow_client::{
    components::{EscrowState, fetch_escrow_state},
    instructions::{
        build_make, build_take, build_cancel,
        MakeAccounts, TakeAccounts, CancelAccounts,
    },
    types::{PROGRAM_ID, EscrowCreated, EscrowExchanged, EscrowCancelled},
    get_escrow_state_pda,
};
use naclac_client::*;
use naclac_client::utils::{
    create_mint_with_program, create_ata_with_program, mint_to_with_program,
    load_node_wallet, TOKEN_PROGRAM_ID,
};

#[test]
fn test_escrow_lifecycle() {
    let name = "SPL Token";
    println!("\nðŸš€ [{}] Starting Escrow Lifecycle Test...", name);

    // 1. Setup the provider
    let cluster = "litesvm";
    let payer = load_node_wallet().expect("Failed to load local Solana keypair");
    let provider = NaclacProvider::new(cluster, payer).expect("Failed to construct NaclacProvider");
    let token_program_id = TOKEN_PROGRAM_ID;

    // Maker and Taker are both the provider's payer (node wallet)
    let maker_address = provider.payer.address();

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

    // Mint initial tokens
    let initial_maker_a = 1_000_000_000u64;
    let initial_taker_b = 2_000_000_000u64;
    mint_to_with_program(&provider, &mint_a, &maker_token_account_a, &provider.payer, initial_maker_a, &token_program_id)
        .expect("Failed to mint to maker ATA A");
    mint_to_with_program(&provider, &mint_b, &maker_token_account_b, &provider.payer, initial_taker_b, &token_program_id)
        .expect("Failed to mint to maker ATA B");

    let fetcher = AccountFetcher::new(&provider);

    // =========================================================================
    // FLOW 1: Make & Cancel
    // =========================================================================
    let seed_cancel = 789012u64;
    let (escrow_pda_cancel, escrow_bump_cancel) = get_escrow_state_pda(&PROGRAM_ID, &maker_address, seed_cancel);

    let mut already_initialized_cancel = false;
    if let Ok(account) = provider.get_account(&escrow_pda_cancel) {
        if account.data.len() >= 8 {
            already_initialized_cancel = true;
            println!("   â„¹ï¸ Escrow account for cancel already exists. Skipping Make.");
        }
    }

    let vault_token_account_cancel = create_ata_with_program(&provider, &mint_a, &escrow_pda_cancel, &token_program_id)
        .expect("Failed to create vault ATA for cancel");

    let amount_a_cancel = 400_000_000u64;
    let amount_b_cancel = 800_000_000u64;

    if !already_initialized_cancel {
        println!("ðŸ¤ Sending MAKE transaction for Cancel Flow (A = {}, B = {})...", amount_a_cancel, amount_b_cancel);

        let tx_meta = build_make(
            &provider,
            PROGRAM_ID,
            seed_cancel,
            escrow_bump_cancel,
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

        println!("   ðŸ“Š Make Compute Units (Cancel Flow): {}", tx_meta.compute_units_consumed);
    }

    // Verify on-chain escrow state and vault balance
    let escrow_state: EscrowState = fetch_escrow_state(&provider, &escrow_pda_cancel)
        .expect("Failed to fetch escrow state");
    assert_eq!(escrow_state.amount_a, amount_a_cancel);

    let vault_balance: TokenAccount = fetcher
        .fetch(&vault_token_account_cancel)
        .expect("Failed to fetch vault token account");
    assert_eq!(vault_balance.amount(), amount_a_cancel);

    // Cancel Escrow
    println!("ðŸ›‘ Sending CANCEL transaction...");

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

    println!("   ðŸ“Š Cancel Compute Units: {}", tx_meta_cancel.compute_units_consumed);

    // Parse and verify EscrowCancelled event
    let cancel_events: Vec<EscrowCancelled> = tx_meta_cancel
        .parse_events_zero_copy()
        .expect("Failed to parse cancel events");
    assert!(!cancel_events.is_empty(), "EscrowCancelled event not emitted");
    assert_eq!(cancel_events[0].maker, maker_address);
    assert_eq!(cancel_events[0].amount_a, amount_a_cancel);

    // Verify balances after CANCEL
    let maker_a_balance: TokenAccount = fetcher
        .fetch(&maker_token_account_a)
        .expect("Failed to fetch maker token account A");
    assert_eq!(maker_a_balance.amount(), initial_maker_a);

    // Verify accounts closed
    assert!(provider.get_account(&escrow_pda_cancel).is_err(), "Escrow state PDA not closed");
    assert!(provider.get_account(&vault_token_account_cancel).is_err(), "Vault token account not closed");
    println!("   âœ… Cancel Flow completed and verified successfully.");

    // =========================================================================
    // FLOW 2: Make & Take
    // =========================================================================
    let seed_take = 123456u64;
    let (escrow_pda_take, escrow_bump_take) = get_escrow_state_pda(&PROGRAM_ID, &maker_address, seed_take);

    let mut already_initialized_take = false;
    if let Ok(account) = provider.get_account(&escrow_pda_take) {
        if account.data.len() >= 8 {
            already_initialized_take = true;
            println!("   â„¹ï¸ Escrow account for take already exists. Skipping Make.");
        }
    }

    let vault_token_account_take = create_ata_with_program(&provider, &mint_a, &escrow_pda_take, &token_program_id)
        .expect("Failed to create vault ATA for take");

    let amount_a_take = 500_000_000u64;
    let amount_b_take = 1_000_000_000u64;

    if !already_initialized_take {
        println!("ðŸ¤ Sending MAKE transaction for Take Flow (A = {}, B = {})...", amount_a_take, amount_b_take);

        let tx_meta = build_make(
            &provider,
            PROGRAM_ID,
            seed_take,
            escrow_bump_take,
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

        println!("   ðŸ“Š Make Compute Units (Take Flow): {}", tx_meta.compute_units_consumed);

        // Parse and verify EscrowCreated event
        let captured_events: Vec<EscrowCreated> = tx_meta
            .parse_events_zero_copy()
            .expect("Failed to parse events");
        assert!(!captured_events.is_empty(), "EscrowCreated event not emitted");
        assert_eq!(captured_events[0].maker, maker_address);
        assert_eq!(captured_events[0].amount_a, amount_a_take);
        assert_eq!(captured_events[0].amount_b, amount_b_take);
    }

    // Fetch and verify EscrowState PDA state
    let escrow_state_take: EscrowState = fetch_escrow_state(&provider, &escrow_pda_take)
        .expect("Failed to fetch escrow state PDA");
    assert_eq!(escrow_state_take.maker, maker_address);
    assert_eq!(escrow_state_take.mint_a, mint_a);
    assert_eq!(escrow_state_take.mint_b, mint_b);
    assert_eq!(escrow_state_take.amount_a, amount_a_take);
    assert_eq!(escrow_state_take.amount_b, amount_b_take);

    // Verify vault balance
    let vault_balance_take: TokenAccount = fetcher
        .fetch(&vault_token_account_take)
        .expect("Failed to fetch vault token account");
    assert_eq!(vault_balance_take.amount(), amount_a_take);

    // Take Escrow
    println!("ðŸŽ¬ Sending TAKE transaction...");

    let tx_meta_take = build_take(
        &provider,
        PROGRAM_ID,
        seed_take,
        TakeAccounts {
            taker: maker_address,
            maker: maker_address,
            mint_a,
            mint_b,
            escrow_state: escrow_pda_take,
            vault_token_account: vault_token_account_take,
            taker_token_account_a: maker_token_account_a,
            taker_token_account_b: maker_token_account_b,
            maker_token_account_b,
            token_program: token_program_id,
            system_program: Address::default(),
        },
    )
    .send_and_confirm()
    .unwrap();

    println!("   ðŸ“Š Take Compute Units: {}", tx_meta_take.compute_units_consumed);

    // Parse and verify EscrowExchanged event
    let take_events: Vec<EscrowExchanged> = tx_meta_take
        .parse_events_zero_copy()
        .expect("Failed to parse take events");
    assert!(!take_events.is_empty(), "EscrowExchanged event not emitted");
    assert_eq!(take_events[0].taker, maker_address);
    assert_eq!(take_events[0].amount_a, amount_a_take);
    assert_eq!(take_events[0].amount_b, amount_b_take);

    // Verify balances after TAKE
    let maker_b_balance: TokenAccount = fetcher
        .fetch(&maker_token_account_b)
        .expect("Failed to fetch maker token account B");
    assert_eq!(maker_b_balance.amount(), initial_taker_b);

    let maker_a_balance: TokenAccount = fetcher
        .fetch(&maker_token_account_a)
        .expect("Failed to fetch maker token account A");
    assert_eq!(maker_a_balance.amount(), initial_maker_a);

    // Verify accounts closed
    assert!(provider.get_account(&escrow_pda_take).is_err(), "Escrow state PDA not closed");
    assert!(provider.get_account(&vault_token_account_take).is_err(), "Vault token account not closed");

    println!("   âœ… Take Flow completed and verified successfully.");
    println!("\nâœ¨ Escrow integration test completed successfully!\n");
}
