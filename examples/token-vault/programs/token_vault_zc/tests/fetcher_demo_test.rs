mod helper;

use token_vault_zc_client::{
    components::{
        Vault, UserAccount, fetch_vault, fetch_user_account, fetch_all_vault, fetch_all_user_account,
    },
    instructions::{
        build_initialize, build_deposit,
        InitializeAccounts, DepositAccounts,
    },
    types::PROGRAM_ID,
    get_vault_account_pda, get_user_account_pda,
};
use naclac_client::*;
use naclac_client::utils::{
    create_mint_with_program, create_ata_with_program, mint_to_with_program, transfer_sol,
};
use std::time::SystemTime;

#[test]
fn test_fetcher_demo() {
    println!("\n=================== START RUST ZERO-COPY FETCHER TESTS ===================");

    // 1. Setup the provider pointing to localnet
    let payer = load_node_wallet().expect("Failed to load local Solana keypair");
    let provider = NaclacProvider::new("localnet", payer).expect("Failed to construct NaclacProvider");
    println!("ðŸ”‘ Loaded Payer Wallet: {}", provider.payer.address());

    // Generate random vault IDs using system time to avoid collisions on localnet
    let now = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap()
        .as_secs();
    let vault_id1 = now % 100_000_000 + 5000;
    let vault_id2 = now % 100_000_000 + 6000;
    println!("Dynamic Vault ID 1: {}", vault_id1);
    println!("Dynamic Vault ID 2: {}", vault_id2);

    // 2. Setup secondary signer/payer: user2
    let user2_signer = Keypair::new();
    let user2 = user2_signer.address();
    println!("ðŸ‘¤ User 2: {}", user2);

    // Fund user2 with SOL
    transfer_sol(&provider, &user2, 100_000_000).expect("Failed to fund user2");

    // 3. Setup Mint and Token Accounts
    let token_program_id = naclac_client::utils::TOKEN_PROGRAM_ID;
    let mint_signer = Keypair::new();
    let mint = mint_signer.address();
    println!("ðŸª™ Creating Mint: {}", mint);
    create_mint_with_program(&provider, &mint_signer, &provider.payer.address(), 9, &token_program_id)
        .expect("Failed to create mint");

    // Deriving PDAs
    let program_id = PROGRAM_ID;
    let (vault1_pda, vault1_bump) = get_vault_account_pda(&program_id, vault_id1);
    let (vault2_pda, vault2_bump) = get_vault_account_pda(&program_id, vault_id2);

    let (user1_vault1_pda, user1_vault1_bump) = get_user_account_pda(&program_id, vault_id1, &provider.payer.address(), &mint);
    let (user2_vault1_pda, user2_vault1_bump) = get_user_account_pda(&program_id, vault_id1, &user2, &mint);
    let (user1_vault2_pda, user1_vault2_bump) = get_user_account_pda(&program_id, vault_id2, &provider.payer.address(), &mint);

    // Associated Token Accounts
    let user1_ata = create_ata_with_program(&provider, &mint, &provider.payer.address(), &token_program_id)
        .expect("Failed to create user1 ATA");
    let user2_ata = create_ata_with_program(&provider, &mint, &user2, &token_program_id)
        .expect("Failed to create user2 ATA");

    let vault1_ata = create_ata_with_program(&provider, &mint, &vault1_pda, &token_program_id)
        .expect("Failed to create vault1 ATA");
    let vault2_ata = create_ata_with_program(&provider, &mint, &vault2_pda, &token_program_id)
        .expect("Failed to create vault2 ATA");

    // Mint tokens
    mint_to_with_program(&provider, &mint, &user1_ata, &provider.payer, 1_000_000, &token_program_id)
        .expect("Failed to mint tokens to user1");
    mint_to_with_program(&provider, &mint, &user2_ata, &provider.payer, 1_000_000, &token_program_id)
        .expect("Failed to mint tokens to user2");

    // 4. Send instructions to initialize and deposit
    let system_program = Address::default();

    // Init Vault 1
    build_initialize(
        &provider,
        program_id,
        vault_id1,
        vault1_bump,
        InitializeAccounts {
            payer: provider.payer.address(),
            vault_account: vault1_pda,
            system_program,
        },
    )
    .send_and_confirm()
    .expect("Failed to initialize Vault 1");

    // Init Vault 2
    build_initialize(
        &provider,
        program_id,
        vault_id2,
        vault2_bump,
        InitializeAccounts {
            payer: provider.payer.address(),
            vault_account: vault2_pda,
            system_program,
        },
    )
    .send_and_confirm()
    .expect("Failed to initialize Vault 2");

    // Deposit User 1 into Vault 1
    build_deposit(
        &provider,
        program_id,
        vault_id1,
        5000,
        user1_vault1_bump,
        DepositAccounts {
            payer: provider.payer.address(),
            mint,
            vault_account: vault1_pda,
            vault_token_account: vault1_ata,
            user_token_account: user1_ata,
            user_account: user1_vault1_pda,
            token_program: token_program_id,
            system_program,
        },
    )
    .send_and_confirm()
    .expect("Failed to deposit User 1 into Vault 1");

    // Deposit User 2 into Vault 1
    let provider_user2 = NaclacProvider::new("localnet", user2_signer).expect("Failed to construct NaclacProvider");
    build_deposit(
        &provider_user2,
        program_id,
        vault_id1,
        7000,
        user2_vault1_bump,
        DepositAccounts {
            payer: user2,
            mint,
            vault_account: vault1_pda,
            vault_token_account: vault1_ata,
            user_token_account: user2_ata,
            user_account: user2_vault1_pda,
            token_program: token_program_id,
            system_program,
        },
    )
    .send_and_confirm()
    .expect("Failed to deposit User 2 into Vault 1");

    // Deposit User 1 into Vault 2
    build_deposit(
        &provider,
        program_id,
        vault_id2,
        9000,
        user1_vault2_bump,
        DepositAccounts {
            payer: provider.payer.address(),
            mint,
            vault_account: vault2_pda,
            vault_token_account: vault2_ata,
            user_token_account: user1_ata,
            user_account: user1_vault2_pda,
            token_program: token_program_id,
            system_program,
        },
    )
    .send_and_confirm()
    .expect("Failed to deposit User 1 into Vault 2");

    // 5. Test Account Fetchers in Rust
    let fetcher = AccountFetcher::new(&provider);

    // --- fetch_zero_copy (Single) ---
    println!("\n--- fetch_zero_copy (Single Vault) ---");
    let vault1: Vault = fetch_vault(&provider, &vault1_pda).unwrap();
    println!("Vault 1 state: authority = {}, vault_id = {}, bump = {}", vault1.authority, vault1.vault_id, vault1.bump);

    println!("\n--- fetch_zero_copy (Single UserAccount) ---");
    let user1_vault1: UserAccount = fetch_user_account(&provider, &user1_vault1_pda).unwrap();
    println!("User 1 Vault 1 state: owner = {}, balance = {}, bump = {}", user1_vault1.owner, user1_vault1.balance, user1_vault1.bump);

    // --- fetch_multiple_zero_copy (Multiple) ---
    println!("\n--- fetch_multiple_zero_copy (Multiple Vaults) ---");
    let vaults: Vec<Option<Vault>> = fetcher.fetch_multiple(&[vault1_pda, vault2_pda]).unwrap();
    println!("Fetched multiple vaults count: {}", vaults.len());
    for (i, v_opt) in vaults.iter().enumerate() {
        if let Some(v) = v_opt {
            println!("  Vault {} ID: {}", i + 1, v.vault_id);
        } else {
            println!("  Vault {} was missing/corrupted", i + 1);
        }
    }

    // --- fetch_multiple_zero_copy_as_map (OOP Map Helper) ---
    println!("\n--- fetch_multiple_zero_copy_as_map (OOP Map Helper) ---");
    let mapped_vaults = fetcher.fetch_multiple_as_map::<Vault>(&[vault1_pda, vault2_pda]).unwrap();
    println!("Fetched mapped vaults count: {}", mapped_vaults.len());
    if let Some(Some(v1)) = mapped_vaults.get(&vault1_pda) {
        println!("  Vault 1 mapped ID: {}", v1.vault_id);
    }
    if let Some(Some(v2)) = mapped_vaults.get(&vault2_pda) {
        println!("  Vault 2 mapped ID: {}", v2.vault_id);
    }

    // --- fetch_all_zero_copy (All by program owner) ---
    println!("\n--- fetch_all_zero_copy (All Vaults) ---");
    let all_vaults: Vec<(Address, Vault)> = fetch_all_vault(&provider, &program_id).unwrap();
    println!("All program Vaults count: {}", all_vaults.len());
    for (addr, v) in &all_vaults {
        println!("  Address: {}, Vault ID: {}", addr, v.vault_id);
    }

    println!("\n--- fetch_all_zero_copy (All UserAccounts) ---");
    let all_users: Vec<(Address, UserAccount)> = fetch_all_user_account(&provider, &program_id).unwrap();
    println!("All program UserAccounts count: {}", all_users.len());
    for (addr, u) in &all_users {
        println!("  Address: {}, Owner: {}, Balance: {}", addr, u.owner, u.balance);
    }

    println!("\n=================== END RUST ZERO-COPY FETCHER TESTS ===================");
}
