use naclac_client::*;
use token_creator_client::{
    instructions::{
        build_launch_token,
        LaunchTokenAccounts,
        build_create_mint,
        CreateMintAccounts,
    },
    components::{LaunchRecord, fetch_launch_record},
    types::{PROGRAM_ID as CREATOR_PROGRAM_ID, LaunchTokenArgs},
    get_launch_record_pda,
};
use amm_client::{
    types::PROGRAM_ID as AMM_PROGRAM_ID,
    get_pool_state_pda,
};




#[test]
fn test_launchpad_integration() {
    let cluster = "litesvm";
    let payer = load_node_wallet().expect("Failed to load local Solana keypair");
    let provider = NaclacProvider::new(cluster, payer);
    println!("\n🔑 Loaded Payer Wallet: {}", provider.payer.address());

    // 1. Load the compiled binaries
    let mut so_path_amm = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    so_path_amm.pop(); // programs
    so_path_amm.pop(); // launchpad workspace root
    so_path_amm.push("target/deploy/amm.so");

    let mut so_path_creator = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    so_path_creator.pop(); // programs
    so_path_creator.pop(); // launchpad workspace root
    so_path_creator.push("target/deploy/token_creator.so");

    provider.add_program(&AMM_PROGRAM_ID, so_path_amm.to_str().unwrap()).expect("Failed to load AMM binary");
    provider.add_program(&CREATOR_PROGRAM_ID, so_path_creator.to_str().unwrap()).expect("Failed to load Token Creator binary");
    println!("   📦 Loaded program binaries.");

    // 2. Setup the quote mint (USDC)
    let quote_mint_signer = Keypair::new();
    let quote_mint_address = quote_mint_signer.address();
    create_mint(&provider, &quote_mint_signer, &provider.payer.address(), 6).expect("Failed to create quote mint");
    println!("   🪙 Created Quote Mint: {}", quote_mint_address);

    // Create payer token B account and mint some quote tokens
    let payer_token_b = create_ata(&provider, &quote_mint_address, &provider.payer.address()).expect("Failed to create payer token B account");
    mint_to(&provider, &quote_mint_address, &payer_token_b, &provider.payer, 10_000_000_000).expect("Failed to mint quote tokens");
    println!("   💰 Minted quote tokens to payer.");

    // 3. Derive the new Token A mint PDA of token_creator
    let id = 42u64;
    let id_bytes = id.to_le_bytes();
    let (mint_address, mint_bump) = Address::find_program_address(
        &[b"mint", &id_bytes],
        &CREATOR_PROGRAM_ID,
    );
    println!("   🪙 Derived Token A Mint PDA: {}", mint_address);

    // 5. Derive launch_record and launcher token accounts
    let (launch_record, launch_record_bump) = get_launch_record_pda(&CREATOR_PROGRAM_ID, &provider.payer.address(), id);
    println!("   📝 Derived Launch Record PDA: {}", launch_record);

    // Create the mint PDA on-chain via the program's create_mint instruction
    let decimals = 6u8;
    build_create_mint(
        &provider,
        CREATOR_PROGRAM_ID,
        id,
        mint_bump,
        launch_record_bump,
        decimals,
        CreateMintAccounts {
            payer: provider.payer.address(),
            mint: mint_address,
            launch_record,
            token_program: TOKEN_PROGRAM_ID,
            system_program: SYSTEM_PROGRAM_ID,
        }
    )
    .send_and_confirm()
    .expect("Failed to create mint on-chain");
    println!("   ✅ Mint created on-chain!");

    // 4. Derive pool_state and create vaults/lp_mint owned by pool_state
    let (pool_state, pool_bump) = get_pool_state_pda(&AMM_PROGRAM_ID, &mint_address, &quote_mint_address, id);
    println!("   🌊 Derived AMM Pool State PDA: {}", pool_state);

    let pool_vault_a_signer = Keypair::new();
    let pool_vault_b_signer = Keypair::new();
    let pool_lp_mint_signer = Keypair::new();

    create_token_account(&provider, &pool_vault_a_signer, &mint_address, &pool_state).expect("Failed to create pool vault A");
    create_token_account(&provider, &pool_vault_b_signer, &quote_mint_address, &pool_state).expect("Failed to create pool vault B");
    create_mint(&provider, &pool_lp_mint_signer, &pool_state, 6).expect("Failed to create pool LP mint");
    println!("   🏦 Created AMM vault and LP mint accounts.");

    let launcher_token_a_signer = Keypair::new();
    let launcher_token_b_signer = Keypair::new();
    let launcher_lp_signer = Keypair::new();

    create_token_account(&provider, &launcher_token_a_signer, &mint_address, &launch_record).expect("Failed to create launcher vault A");
    create_token_account(&provider, &launcher_token_b_signer, &quote_mint_address, &launch_record).expect("Failed to create launcher vault B");
    create_token_account(&provider, &launcher_lp_signer, &pool_lp_mint_signer.address(), &launch_record).expect("Failed to create launcher LP vault");
    println!("   🚀 Created launcher vaults.");

    // 6. Execute launch_token CPI
    println!("🔥 Sending launch_token transaction...");
    let decimals = 6u8;
    let amount_token_pool = 1_000_000_000;
    let amount_token_launcher = 500_000_000;
    let amount_quote = 2_000_000_000;

    let args = LaunchTokenArgs {
        id,
        mint_bump,
        launch_record_bump,
        pool_bump,
        decimals,
        amount_token_pool,
        amount_token_launcher,
        amount_quote,
    };

    let tx_meta = build_launch_token(
        &provider,
        CREATOR_PROGRAM_ID,
        args,
        LaunchTokenAccounts {
            payer: provider.payer.address(),
            quote_mint: quote_mint_address,
            mint: mint_address,
            launch_record,
            launcher_token_a: launcher_token_a_signer.address(),
            launcher_token_b: launcher_token_b_signer.address(),
            launcher_lp: launcher_lp_signer.address(),
            payer_token_b,
            pool_state,
            pool_vault_a: pool_vault_a_signer.address(),
            pool_vault_b: pool_vault_b_signer.address(),
            pool_lp_mint: pool_lp_mint_signer.address(),
            amm_program: AMM_PROGRAM_ID,
            token_program: TOKEN_PROGRAM_ID,
            system_program: SYSTEM_PROGRAM_ID,
        },
    )
    .send_and_confirm()
    .unwrap();

    println!("   📊 Compute Units (CUs) consumed: {}", tx_meta.compute_units_consumed);
    if !tx_meta.logs.is_empty() {
        println!("   📜 Program Logs:");
        for log in &tx_meta.logs {
            println!("      {}", log);
        }
    }

    // 7. Verify launch record state using AccountFetcher (zero-copy / POD)
    let fetcher = AccountFetcher::new(&provider);
    let record: LaunchRecord = fetch_launch_record(&provider, &launch_record)
        .expect("Failed to fetch launch record state");
    
    assert_eq!(record.creator, provider.payer.address());
    assert_eq!(record.mint, mint_address);
    assert_eq!(record.amount_token, amount_token_pool);
    assert_eq!(record.amount_quote, amount_quote);
    println!("   ✅ State verified successfully!");

    println!("\n✨ Launchpad integration test completed successfully!\n");
}
