use naclac_client::*;
use amm_client::{
    types::PROGRAM_ID as AMM_PROGRAM_ID,
    get_pool_state_pda,
    instructions::{
        build_initialize,
        InitializeAccounts,
        build_add_liquidity,
        AddLiquidityAccounts,
        build_swap,
        SwapAccounts,
        build_remove_liquidity,
        RemoveLiquidityAccounts,
    },
};

#[test]
fn test_amm_full_lifecycle() {
    let cluster = "litesvm";
    let payer = load_node_wallet().expect("Failed to load local Solana keypair");
    let provider = NaclacProvider::new(cluster, payer);
    println!("\n🔑 Loaded Payer Wallet: {}", provider.payer.address());

    // 1. Load compiled AMM binary
    let mut so_path_amm = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    so_path_amm.pop(); // programs
    so_path_amm.pop(); // launchpad workspace root
    so_path_amm.push("target/deploy/amm.so");

    provider.add_program(&AMM_PROGRAM_ID, so_path_amm.to_str().unwrap()).expect("Failed to load AMM binary");
    println!("   📦 Loaded AMM program binary.");

    // 2. Setup Token A Mint and Token B Mint
    let token_a_mint_signer = Keypair::new();
    let token_a_mint = token_a_mint_signer.address();
    create_mint(&provider, &token_a_mint_signer, &provider.payer.address(), 6).expect("Failed to create Token A mint");
    println!("   🪙 Created Token A Mint: {}", token_a_mint);

    let token_b_mint_signer = Keypair::new();
    let token_b_mint = token_b_mint_signer.address();
    create_mint(&provider, &token_b_mint_signer, &provider.payer.address(), 6).expect("Failed to create Token B mint");
    println!("   🪙 Created Token B Mint: {}", token_b_mint);

    // Create user token accounts and mint initial supply
    let user_token_a = create_ata(&provider, &token_a_mint, &provider.payer.address()).expect("Failed to create user Token A ATA");
    let user_token_b = create_ata(&provider, &token_b_mint, &provider.payer.address()).expect("Failed to create user Token B ATA");

    mint_to(&provider, &token_a_mint, &user_token_a, &provider.payer, 10_000_000_000).expect("Failed to mint Token A to user");
    mint_to(&provider, &token_b_mint, &user_token_b, &provider.payer, 10_000_000_000).expect("Failed to mint Token B to user");
    println!("   💰 Minted initial tokens to user.");

    // 3. Derive pool_state and setup vaults
    let id = 100u64;
    let (pool_state, pool_bump) = get_pool_state_pda(&AMM_PROGRAM_ID, &token_a_mint, &token_b_mint, id);
    println!("   🌊 Derived AMM Pool State PDA: {}", pool_state);

    let pool_vault_a_signer = Keypair::new();
    let pool_vault_b_signer = Keypair::new();
    let pool_lp_mint_signer = Keypair::new();

    create_token_account(&provider, &pool_vault_a_signer, &token_a_mint, &pool_state).expect("Failed to create pool vault A");
    create_token_account(&provider, &pool_vault_b_signer, &token_b_mint, &pool_state).expect("Failed to create pool vault B");
    create_mint(&provider, &pool_lp_mint_signer, &pool_state, 6).expect("Failed to create pool LP mint");
    println!("   🏦 Created AMM vault and LP mint accounts.");

    let user_lp = create_ata(&provider, &pool_lp_mint_signer.address(), &provider.payer.address()).expect("Failed to create user LP ATA");
    println!("   🎟️ Created user LP ATA.");

    // 4. Initialize Pool
    println!("🔥 Sending initialize pool transaction...");
    let amount_a = 2_000_000_000;
    let amount_b = 4_000_000_000;
    let tx_meta = build_initialize(
        &provider,
        AMM_PROGRAM_ID,
        id,
        pool_bump,
        amount_a,
        amount_b,
        InitializeAccounts {
            payer: provider.payer.address(),
            token_a_mint,
            token_b_mint,
            pool_state,
            vault_a: pool_vault_a_signer.address(),
            vault_b: pool_vault_b_signer.address(),
            lp_mint: pool_lp_mint_signer.address(),
            depositor_token_a: user_token_a,
            depositor_token_b: user_token_b,
            depositor_lp: user_lp,
            depositor_authority: provider.payer.address(),
            token_program: TOKEN_PROGRAM_ID,
            system_program: SYSTEM_PROGRAM_ID,
        }
    )
    .send_and_confirm()
    .expect("Failed to initialize pool");
    println!("   ✅ Pool initialized!");
    println!("   📊 Compute Units (CUs) consumed: {}", tx_meta.compute_units_consumed);
    if !tx_meta.logs.is_empty() {
        println!("   📜 Program Logs:");
        for log in &tx_meta.logs {
            println!("      {}", log);
        }
    }

    // Verify initial state
    let fetcher = AccountFetcher::new(&provider);
    let vault_a_balance = fetcher.fetch::<TokenAccount>(&pool_vault_a_signer.address()).expect("Failed to get vault A balance").amount();
    let vault_b_balance = fetcher.fetch::<TokenAccount>(&pool_vault_b_signer.address()).expect("Failed to get vault B balance").amount();
    let user_lp_balance = fetcher.fetch::<TokenAccount>(&user_lp).expect("Failed to get user LP balance").amount();

    assert_eq!(vault_a_balance, amount_a);
    assert_eq!(vault_b_balance, amount_b);
    // Initial LP tokens minted should match sqrt(amount_a * amount_b)
    let expected_lp = ((amount_a as f64) * (amount_b as f64)).sqrt() as u64;
    assert_eq!(user_lp_balance, expected_lp);
    println!("   📊 Initial balances verified. LP minted: {}", user_lp_balance);

    // 5. Add Liquidity
    println!("🔥 Adding liquidity...");
    let add_amount_a = 500_000_000;
    let add_amount_b = 1_000_000_000;
    let tx_meta = build_add_liquidity(
        &provider,
        AMM_PROGRAM_ID,
        add_amount_a,
        add_amount_b,
        AddLiquidityAccounts {
            user: provider.payer.address(),
            pool_state,
            token_a_mint,
            token_b_mint,
            vault_a: pool_vault_a_signer.address(),
            vault_b: pool_vault_b_signer.address(),
            lp_mint: pool_lp_mint_signer.address(),
            user_token_a,
            user_token_b,
            user_lp,
            token_program: TOKEN_PROGRAM_ID,
        }
    )
    .send_and_confirm()
    .expect("Failed to add liquidity");
    println!("   ✅ Liquidity added successfully!");
    println!("   📊 Compute Units (CUs) consumed: {}", tx_meta.compute_units_consumed);
    if !tx_meta.logs.is_empty() {
        println!("   📜 Program Logs:");
        for log in &tx_meta.logs {
            println!("      {}", log);
        }
    }

    let vault_a_balance_post_add = fetcher.fetch::<TokenAccount>(&pool_vault_a_signer.address()).unwrap().amount();
    let user_lp_balance_post_add = fetcher.fetch::<TokenAccount>(&user_lp).unwrap().amount();
    assert_eq!(vault_a_balance_post_add, amount_a + add_amount_a);
    assert!(user_lp_balance_post_add > user_lp_balance);
    println!("   📊 Post-add balances verified. LP count: {}", user_lp_balance_post_add);

    // 6. Swap Token A for Token B
    println!("🔥 Executing swap...");
    let swap_in = 100_000_000;
    let min_out = 150_000_000; // Expected output should be around 196M based on Constant Product
    let tx_meta = build_swap(
        &provider,
        AMM_PROGRAM_ID,
        swap_in,
        min_out,
        SwapAccounts {
            user: provider.payer.address(),
            pool_state,
            token_a_mint,
            token_b_mint,
            pool_source_vault: pool_vault_a_signer.address(),
            pool_destination_vault: pool_vault_b_signer.address(),
            user_source_token: user_token_a,
            user_destination_token: user_token_b,
            token_program: TOKEN_PROGRAM_ID,
        }
    )
    .send_and_confirm()
    .expect("Failed to execute swap");
    println!("   ✅ Swap executed successfully!");
    println!("   📊 Compute Units (CUs) consumed: {}", tx_meta.compute_units_consumed);
    if !tx_meta.logs.is_empty() {
        println!("   📜 Program Logs:");
        for log in &tx_meta.logs {
            println!("      {}", log);
        }
    }

    let vault_a_balance_post_swap = fetcher.fetch::<TokenAccount>(&pool_vault_a_signer.address()).unwrap().amount();
    assert_eq!(vault_a_balance_post_swap, amount_a + add_amount_a + swap_in);
    println!("   📊 Post-swap vault A balance: {}", vault_a_balance_post_swap);

    // 7. Remove Liquidity
    println!("🔥 Removing liquidity...");
    let remove_lp_amount = 500_000_000;
    let tx_meta = build_remove_liquidity(
        &provider,
        AMM_PROGRAM_ID,
        remove_lp_amount,
        RemoveLiquidityAccounts {
            user: provider.payer.address(),
            pool_state,
            token_a_mint,
            token_b_mint,
            vault_a: pool_vault_a_signer.address(),
            vault_b: pool_vault_b_signer.address(),
            lp_mint: pool_lp_mint_signer.address(),
            user_token_a,
            user_token_b,
            user_lp,
            token_program: TOKEN_PROGRAM_ID,
        }
    )
    .send_and_confirm()
    .expect("Failed to remove liquidity");
    println!("   ✅ Liquidity removed successfully!");
    println!("   📊 Compute Units (CUs) consumed: {}", tx_meta.compute_units_consumed);
    if !tx_meta.logs.is_empty() {
        println!("   📜 Program Logs:");
        for log in &tx_meta.logs {
            println!("      {}", log);
        }
    }

    let user_lp_balance_final = fetcher.fetch::<TokenAccount>(&user_lp).unwrap().amount();
    assert_eq!(user_lp_balance_final, user_lp_balance_post_add - remove_lp_amount);
    println!("   📊 Final LP Balance: {}", user_lp_balance_final);
    println!("\n✨ AMM full lifecycle test completed successfully!\n");
}
