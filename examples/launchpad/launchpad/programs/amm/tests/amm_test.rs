//! AMM integration tests: the full lifecycle over every pairing of the Token
//! and Token-2022 programs, slippage limits, mint and token-program checks.

use {
    amm_client::{
        get_pool_state_pda,
        instructions::{
            build_add_liquidity, build_initialize, build_remove_liquidity, build_swap,
            AddLiquidityAccounts, InitializeAccounts, RemoveLiquidityAccounts, SwapAccounts,
        },
        types::PROGRAM_ID as AMM_PROGRAM_ID,
    },
    naclac_client::{
        utils::{
            create_ata_with_program, create_mint_with_program, load_node_wallet,
            mint_to_with_program, ASSOCIATED_TOKEN_PROGRAM_ID, SYSTEM_PROGRAM_ID,
            TOKEN_2022_PROGRAM_ID, TOKEN_PROGRAM_ID,
        },
        *,
    },
};

const INVALID_MINT: u32 = 6000;
const ZERO_AMOUNT: u32 = 6002;
const SLIPPAGE_EXCEEDED: u32 = 6003;
const INSUFFICIENT_LIQUIDITY: u32 = 6004;
const UNSUPPORTED_MINT_EXTENSION: u32 = 6005;
const TOKEN_A_MINT_PROGRAM_MISMATCH: u32 = 10_209;

const USER_BALANCE: u64 = 10_000_000_000;
const SEED_AMOUNT_A: u64 = 2_000_000_000;
const SEED_AMOUNT_B: u64 = 4_000_000_000;
const SEED_LP: u64 = 2_828_427_124;

const PERMANENT_DELEGATE_EXTENSION: (u16, usize) = (12, 32);
const TRANSFER_FEE_CONFIG_EXTENSION: (u16, usize) = (1, 108);
const METADATA_POINTER_EXTENSION: (u16, usize) = (18, 64);

struct TestMint {
    address: Address,
    program: Address,
    user_account: Address,
}

struct Pool {
    user: Address,
    token_a_mint: Address,
    token_b_mint: Address,
    token_program_a: Address,
    token_program_b: Address,
    pool_state: Address,
    lp_mint: Address,
    vault_a: Address,
    vault_b: Address,
    user_token_a: Address,
    user_token_b: Address,
    user_lp: Address,
}

impl Pool {
    fn add_accounts(&self) -> AddLiquidityAccounts {
        AddLiquidityAccounts {
            user: self.user,
            pool_state: self.pool_state,
            token_a_mint: self.token_a_mint,
            token_b_mint: self.token_b_mint,
            vault_a: self.vault_a,
            vault_b: self.vault_b,
            lp_mint: self.lp_mint,
            user_token_a: self.user_token_a,
            user_token_b: self.user_token_b,
            user_lp: self.user_lp,
            token_program_a: self.token_program_a,
            token_program_b: self.token_program_b,
            lp_token_program: TOKEN_PROGRAM_ID,
        }
    }

    fn remove_accounts(&self) -> RemoveLiquidityAccounts {
        RemoveLiquidityAccounts {
            user: self.user,
            pool_state: self.pool_state,
            token_a_mint: self.token_a_mint,
            token_b_mint: self.token_b_mint,
            vault_a: self.vault_a,
            vault_b: self.vault_b,
            lp_mint: self.lp_mint,
            user_token_a: self.user_token_a,
            user_token_b: self.user_token_b,
            user_lp: self.user_lp,
            token_program_a: self.token_program_a,
            token_program_b: self.token_program_b,
            lp_token_program: TOKEN_PROGRAM_ID,
        }
    }

    fn swap_accounts(&self, a_for_b: bool) -> SwapAccounts {
        let (source_vault, destination_vault, source_token, destination_token) = if a_for_b {
            (self.vault_a, self.vault_b, self.user_token_a, self.user_token_b)
        } else {
            (self.vault_b, self.vault_a, self.user_token_b, self.user_token_a)
        };
        SwapAccounts {
            user: self.user,
            pool_state: self.pool_state,
            token_a_mint: self.token_a_mint,
            token_b_mint: self.token_b_mint,
            pool_source_vault: source_vault,
            pool_destination_vault: destination_vault,
            user_source_token: source_token,
            user_destination_token: destination_token,
            token_program_a: self.token_program_a,
            token_program_b: self.token_program_b,
        }
    }
}

struct Env {
    provider: NaclacProvider,
    lo: TestMint,
    hi: TestMint,
}

fn provider() -> NaclacProvider {
    let payer = load_node_wallet().expect("Failed to load local Solana keypair");
    NaclacProvider::new("litesvm", payer).expect("Failed to construct NaclacProvider")
}

fn sorted_signers() -> (Keypair, Keypair) {
    let (x, y) = (Keypair::new(), Keypair::new());
    if x.address().to_bytes() < y.address().to_bytes() {
        (x, y)
    } else {
        (y, x)
    }
}

/// An initialized Token-2022 mint with one extension of the given TLV type and
/// value length (zero-filled), mint authority `authority`.
fn token_2022_mint_data(authority: &Address, extension: (u16, usize)) -> Vec<u8> {
    let (extension_type, extension_len) = extension;
    let mut data = vec![0u8; 82];
    data[0..4].copy_from_slice(&1u32.to_le_bytes());
    data[4..36].copy_from_slice(authority.as_ref());
    data[44] = 6;
    data[45] = 1;
    data.resize(165, 0);
    data.push(1);
    data.extend_from_slice(&extension_type.to_le_bytes());
    data.extend_from_slice(&(extension_len as u16).to_le_bytes());
    data.resize(data.len() + extension_len, 0);
    data
}

fn fund(provider: &NaclacProvider, address: Address, program: Address) -> TestMint {
    let user_account =
        create_ata_with_program(provider, &address, &provider.payer.address(), &program)
            .expect("Failed to create user token account");
    mint_to_with_program(
        provider,
        &address,
        &user_account,
        &provider.payer,
        USER_BALANCE,
        &program,
    )
    .expect("Failed to mint to user");
    TestMint { address, program, user_account }
}

fn standard_mint(provider: &NaclacProvider, signer: &Keypair, program: &Address) -> TestMint {
    create_mint_with_program(provider, signer, &provider.payer.address(), 6, program)
        .expect("Failed to create mint");
    fund(provider, signer.address(), *program)
}

impl Env {
    /// Two mints in ascending address order, `lo` under `program_lo` and `hi`
    /// under `program_hi`, each funded in the payer's token account.
    fn new(program_lo: &Address, program_hi: &Address) -> Env {
        let provider = provider();
        let (lo_signer, hi_signer) = sorted_signers();
        let lo = standard_mint(&provider, &lo_signer, program_lo);
        let hi = standard_mint(&provider, &hi_signer, program_hi);
        Env { provider, lo, hi }
    }

    /// As `new`, but `hi` is a Token-2022 mint carrying one extension.
    fn with_hi_extension(program_lo: &Address, extension: (u16, usize)) -> Env {
        let provider = provider();
        let (lo_signer, hi_signer) = sorted_signers();
        let lo = standard_mint(&provider, &lo_signer, program_lo);
        let data = token_2022_mint_data(&provider.payer.address(), extension);
        provider
            .set_account(&hi_signer.address(), data, &TOKEN_2022_PROGRAM_ID, 10_000_000)
            .expect("Failed to write the extended mint");
        let hi = fund(&provider, hi_signer.address(), TOKEN_2022_PROGRAM_ID);
        Env { provider, lo, hi }
    }

    /// Every address a pool over `a` and `b` with pool number `id` uses, and
    /// the canonical bump of each: pool, LP mint, vault A, vault B, and the
    /// payer's LP associated token account.
    fn derive(&self, a: &TestMint, b: &TestMint, id: u64) -> (Pool, [u8; 5]) {
        let user = self.provider.payer.address();
        let (pool_state, pool_bump) =
            get_pool_state_pda(&AMM_PROGRAM_ID, &a.address, &b.address, id);
        let (lp_mint, lp_mint_bump) =
            Address::find_program_address(&[&b"lp_mint"[..], pool_state.as_ref()], &AMM_PROGRAM_ID);
        let vault = |mint: &Address| {
            Address::find_program_address(
                &[&b"vault"[..], pool_state.as_ref(), mint.as_ref()],
                &AMM_PROGRAM_ID,
            )
        };
        let (vault_a, vault_a_bump) = vault(&a.address);
        let (vault_b, vault_b_bump) = vault(&b.address);
        let (user_lp, user_lp_bump) = Address::find_program_address(
            &[user.as_ref(), TOKEN_PROGRAM_ID.as_ref(), lp_mint.as_ref()],
            &ASSOCIATED_TOKEN_PROGRAM_ID,
        );
        let pool = Pool {
            user,
            token_a_mint: a.address,
            token_b_mint: b.address,
            token_program_a: a.program,
            token_program_b: b.program,
            pool_state,
            lp_mint,
            vault_a,
            vault_b,
            user_token_a: a.user_account,
            user_token_b: b.user_account,
            user_lp,
        };
        (pool, [pool_bump, lp_mint_bump, vault_a_bump, vault_b_bump, user_lp_bump])
    }

    /// The smallest pool number for which the pool, LP mint, both vaults and
    /// the payer's LP account all sit at bump 255, so every address search
    /// ends on its first attempt. Any pool number works; this one is cheapest.
    fn first_bump_id(&self, a: &TestMint, b: &TestMint) -> u64 {
        (0u64..)
            .find(|&id| self.derive(a, b, id).1 == [255; 5])
            .expect("a pool number with every bump at 255 exists")
    }

    /// The smallest pool number for which the pool address and the payer's LP
    /// account both sit below bump 255, so their searches take extra attempts.
    fn first_low_bump_id(&self, a: &TestMint, b: &TestMint) -> u64 {
        (0u64..)
            .find(|&id| {
                let bumps = self.derive(a, b, id).1;
                bumps[0] != 255 && bumps[4] != 255
            })
            .expect("a pool number with the pool and LP account below 255 exists")
    }

    fn pool_addresses(&self, a: &TestMint, b: &TestMint) -> Pool {
        self.derive(a, b, self.first_bump_id(a, b)).0
    }

    fn init(
        &self,
        a: &TestMint,
        b: &TestMint,
        amount_a: u64,
        amount_b: u64,
    ) -> (Pool, Result<NaclacTransactionMetadata, NaclacClientError>) {
        self.init_with_id(a, b, self.first_bump_id(a, b), amount_a, amount_b)
    }

    fn init_with_id(
        &self,
        a: &TestMint,
        b: &TestMint,
        id: u64,
        amount_a: u64,
        amount_b: u64,
    ) -> (Pool, Result<NaclacTransactionMetadata, NaclacClientError>) {
        let user = self.provider.payer.address();
        let (pool, _) = self.derive(a, b, id);
        let pool_state = pool.pool_state;
        let lp_mint = pool.lp_mint;
        let result = build_initialize(
            &self.provider,
            AMM_PROGRAM_ID,
            id,
            amount_a,
            amount_b,
            InitializeAccounts {
                payer: user,
                token_a_mint: pool.token_a_mint,
                token_b_mint: pool.token_b_mint,
                pool_state,
                lp_mint,
                vault_a: pool.vault_a,
                vault_b: pool.vault_b,
                depositor_token_a: pool.user_token_a,
                depositor_token_b: pool.user_token_b,
                depositor_lp: pool.user_lp,
                token_program_a: pool.token_program_a,
                token_program_b: pool.token_program_b,
                lp_token_program: TOKEN_PROGRAM_ID,
                associated_token_program: ASSOCIATED_TOKEN_PROGRAM_ID,
                system_program: SYSTEM_PROGRAM_ID,
            },
        )
        .send_and_confirm();
        (pool, result)
    }

    fn initialized(&self) -> (Pool, NaclacTransactionMetadata) {
        let (pool, result) = self.init(&self.lo, &self.hi, SEED_AMOUNT_A, SEED_AMOUNT_B);
        (pool, result.expect("Failed to initialize pool"))
    }

    fn balance(&self, address: &Address) -> u64 {
        AccountFetcher::new(&self.provider)
            .fetch::<TokenAccount>(address)
            .expect("Failed to read token account")
            .amount()
    }
}

fn assert_custom_code(result: Result<NaclacTransactionMetadata, NaclacClientError>, expected: u32) {
    match result {
        Err(NaclacClientError::TransactionFailed {
            instruction_err: InstructionError::Custom(code),
            ..
        }) => assert_eq!(code, expected, "wrong custom error code"),
        other => panic!("expected a Custom({expected}) transaction failure, got {other:?}"),
    }
}

fn print_meta(label: &str, meta: &NaclacTransactionMetadata) {
    println!("   ✅ {label}");
    println!("   📊 Compute Units (CUs) consumed: {}", meta.compute_units_consumed);
    for log in &meta.logs {
        println!("      {log}");
    }
}

fn lifecycle(env: Env) {
    let (pool, tx_meta) = env.initialized();
    print_meta("Pool initialized!", &tx_meta);

    assert_eq!(env.balance(&pool.vault_a), SEED_AMOUNT_A);
    assert_eq!(env.balance(&pool.vault_b), SEED_AMOUNT_B);
    let expected_lp = ((SEED_AMOUNT_A as f64) * (SEED_AMOUNT_B as f64)).sqrt() as u64;
    let user_lp_balance = env.balance(&pool.user_lp);
    assert_eq!(user_lp_balance, expected_lp);

    let add_amount_a = 500_000_000;
    let add_amount_b = 1_000_000_000;
    let tx_meta = build_add_liquidity(
        &env.provider,
        AMM_PROGRAM_ID,
        add_amount_a,
        add_amount_b,
        707_106_781,
        pool.add_accounts(),
    )
    .send_and_confirm()
    .expect("Failed to add liquidity");
    print_meta("Liquidity added successfully!", &tx_meta);
    assert_eq!(env.balance(&pool.vault_a), SEED_AMOUNT_A + add_amount_a);
    let user_lp_balance_post_add = env.balance(&pool.user_lp);
    assert!(user_lp_balance_post_add > user_lp_balance);

    let swap_in = 100_000_000;
    let tx_meta = build_swap(
        &env.provider,
        AMM_PROGRAM_ID,
        swap_in,
        150_000_000,
        pool.swap_accounts(true),
    )
    .send_and_confirm()
    .expect("Failed to execute swap");
    print_meta("Swap executed successfully!", &tx_meta);
    assert_eq!(env.balance(&pool.vault_a), SEED_AMOUNT_A + add_amount_a + swap_in);

    let tx_meta = build_swap(
        &env.provider,
        AMM_PROGRAM_ID,
        swap_in,
        1,
        pool.swap_accounts(false),
    )
    .send_and_confirm()
    .expect("Failed to execute the reverse swap");
    print_meta("Reverse swap executed successfully!", &tx_meta);

    let remove_lp_amount = 500_000_000;
    let tx_meta = build_remove_liquidity(
        &env.provider,
        AMM_PROGRAM_ID,
        remove_lp_amount,
        1,
        1,
        pool.remove_accounts(),
    )
    .send_and_confirm()
    .expect("Failed to remove liquidity");
    print_meta("Liquidity removed successfully!", &tx_meta);
    assert_eq!(env.balance(&pool.user_lp), user_lp_balance_post_add - remove_lp_amount);
}

#[test]
fn test_amm_full_lifecycle() {
    lifecycle(Env::new(&TOKEN_PROGRAM_ID, &TOKEN_PROGRAM_ID));
}

#[test]
fn lifecycle_with_two_token_2022_mints() {
    lifecycle(Env::new(&TOKEN_2022_PROGRAM_ID, &TOKEN_2022_PROGRAM_ID));
}

#[test]
fn lifecycle_with_a_classic_mint_then_a_token_2022_mint() {
    lifecycle(Env::new(&TOKEN_PROGRAM_ID, &TOKEN_2022_PROGRAM_ID));
}

#[test]
fn lifecycle_with_a_token_2022_mint_then_a_classic_mint() {
    lifecycle(Env::new(&TOKEN_2022_PROGRAM_ID, &TOKEN_PROGRAM_ID));
}

#[test]
fn lifecycle_accepts_a_token_2022_mint_with_a_metadata_pointer() {
    lifecycle(Env::with_hi_extension(&TOKEN_PROGRAM_ID, METADATA_POINTER_EXTENSION));
}

#[test]
fn initialize_rejects_a_mint_with_a_permanent_delegate() {
    let env = Env::with_hi_extension(&TOKEN_PROGRAM_ID, PERMANENT_DELEGATE_EXTENSION);
    let (_, result) = env.init(&env.lo, &env.hi, SEED_AMOUNT_A, SEED_AMOUNT_B);
    assert_custom_code(result, UNSUPPORTED_MINT_EXTENSION);
}

#[test]
fn initialize_rejects_a_mint_with_a_transfer_fee() {
    let env = Env::with_hi_extension(&TOKEN_PROGRAM_ID, TRANSFER_FEE_CONFIG_EXTENSION);
    let (_, result) = env.init(&env.lo, &env.hi, SEED_AMOUNT_A, SEED_AMOUNT_B);
    assert!(
        result.is_err(),
        "a transfer-fee mint needs token-account space the vault does not have"
    );
}

#[test]
fn initialize_rejects_unordered_mints() {
    let env = Env::new(&TOKEN_PROGRAM_ID, &TOKEN_PROGRAM_ID);
    let (_, result) = env.init(&env.hi, &env.lo, SEED_AMOUNT_A, SEED_AMOUNT_B);
    assert_custom_code(result, INVALID_MINT);
}

#[test]
fn initialize_rejects_the_same_mint_twice() {
    let env = Env::new(&TOKEN_PROGRAM_ID, &TOKEN_PROGRAM_ID);
    let (_, result) = env.init(&env.lo, &env.lo, SEED_AMOUNT_A, SEED_AMOUNT_B);
    assert!(result.is_err(), "a pool of one mint against itself must be rejected");
}

#[test]
fn initialize_rejects_a_zero_amount() {
    let env = Env::new(&TOKEN_PROGRAM_ID, &TOKEN_PROGRAM_ID);
    let (_, result) = env.init(&env.lo, &env.hi, 0, SEED_AMOUNT_B);
    assert_custom_code(result, ZERO_AMOUNT);
}

#[test]
fn initialize_rejects_a_pool_that_already_exists() {
    let env = Env::new(&TOKEN_PROGRAM_ID, &TOKEN_PROGRAM_ID);
    env.initialized();
    let (_, result) = env.init(&env.lo, &env.hi, SEED_AMOUNT_A, SEED_AMOUNT_B);
    assert!(result.is_err(), "a second pool at the same address must be rejected");
}

#[test]
fn initialize_accepts_a_pool_number_whose_bumps_are_below_255() {
    let env = Env::new(&TOKEN_PROGRAM_ID, &TOKEN_PROGRAM_ID);
    let id = env.first_low_bump_id(&env.lo, &env.hi);
    let (_, result) = env.init_with_id(&env.lo, &env.hi, id, SEED_AMOUNT_A, SEED_AMOUNT_B);
    result.expect("any pool number must work; the program finds the canonical bump itself");
}

#[test]
fn a_pool_number_with_every_bump_at_255_costs_fewer_compute_units() {
    let env = Env::new(&TOKEN_PROGRAM_ID, &TOKEN_PROGRAM_ID);
    let ground_id = env.first_bump_id(&env.lo, &env.hi);
    let low_id = env.first_low_bump_id(&env.lo, &env.hi);
    let (_, ground) = env.init_with_id(&env.lo, &env.hi, ground_id, SEED_AMOUNT_A, SEED_AMOUNT_B);
    let (_, low) = env.init_with_id(&env.lo, &env.hi, low_id, SEED_AMOUNT_A, SEED_AMOUNT_B);
    let ground_cu = ground.expect("the ground pool number must work").compute_units_consumed;
    let low_cu = low.expect("the low-bump pool number must work").compute_units_consumed;
    println!("   📊 bumps all 255: {ground_cu} CUs, pool and LP account below 255: {low_cu} CUs");
    assert!(ground_cu < low_cu);
}

#[test]
fn initialize_succeeds_when_the_pda_addresses_were_prefunded() {
    let env = Env::new(&TOKEN_PROGRAM_ID, &TOKEN_PROGRAM_ID);
    let pool = env.pool_addresses(&env.lo, &env.hi);
    for address in [pool.pool_state, pool.lp_mint, pool.vault_a, pool.vault_b] {
        env.provider
            .set_account_lamports(&address, 1)
            .expect("Failed to prefund an address");
    }
    let (_, result) = env.init(&env.lo, &env.hi, SEED_AMOUNT_A, SEED_AMOUNT_B);
    result.expect("a pre-funded address must not block pool creation");
}

#[test]
fn add_liquidity_enforces_the_minimum_lp_out() {
    let env = Env::new(&TOKEN_PROGRAM_ID, &TOKEN_PROGRAM_ID);
    let (pool, _) = env.initialized();

    let too_greedy = build_add_liquidity(
        &env.provider,
        AMM_PROGRAM_ID,
        500_000_000,
        1_000_000_000,
        707_106_782,
        pool.add_accounts(),
    )
    .send_and_confirm();
    assert_custom_code(too_greedy, SLIPPAGE_EXCEEDED);

    build_add_liquidity(
        &env.provider,
        AMM_PROGRAM_ID,
        500_000_000,
        1_000_000_000,
        707_106_781,
        pool.add_accounts(),
    )
    .send_and_confirm()
    .expect("a deposit minting exactly the minimum must succeed");
}

#[test]
fn add_liquidity_rejects_a_deposit_that_mints_no_shares() {
    let env = Env::new(&TOKEN_PROGRAM_ID, &TOKEN_PROGRAM_ID);
    let (pool, _) = env.initialized();
    let result =
        build_add_liquidity(&env.provider, AMM_PROGRAM_ID, 0, 0, 0, pool.add_accounts())
            .send_and_confirm();
    assert_custom_code(result, ZERO_AMOUNT);
}

#[test]
fn add_liquidity_rejects_a_token_program_that_does_not_own_the_mint() {
    let env = Env::new(&TOKEN_PROGRAM_ID, &TOKEN_PROGRAM_ID);
    let (pool, _) = env.initialized();
    let mut accounts = pool.add_accounts();
    accounts.token_program_a = TOKEN_2022_PROGRAM_ID;
    let result = build_add_liquidity(
        &env.provider,
        AMM_PROGRAM_ID,
        500_000_000,
        1_000_000_000,
        1,
        accounts,
    )
    .send_and_confirm();
    assert_custom_code(result, TOKEN_A_MINT_PROGRAM_MISMATCH);
}

#[test]
fn remove_liquidity_enforces_the_minimum_amounts_out() {
    let env = Env::new(&TOKEN_PROGRAM_ID, &TOKEN_PROGRAM_ID);
    let (pool, _) = env.initialized();

    let lp_amount = 1_000_000_000u64;
    let supply = u128::from(SEED_LP);
    let expected_a = (u128::from(SEED_AMOUNT_A) * u128::from(lp_amount) / supply) as u64;
    let expected_b = (u128::from(SEED_AMOUNT_B) * u128::from(lp_amount) / supply) as u64;

    let a_too_high = build_remove_liquidity(
        &env.provider,
        AMM_PROGRAM_ID,
        lp_amount,
        expected_a + 1,
        1,
        pool.remove_accounts(),
    )
    .send_and_confirm();
    assert_custom_code(a_too_high, SLIPPAGE_EXCEEDED);

    let b_too_high = build_remove_liquidity(
        &env.provider,
        AMM_PROGRAM_ID,
        lp_amount,
        1,
        expected_b + 1,
        pool.remove_accounts(),
    )
    .send_and_confirm();
    assert_custom_code(b_too_high, SLIPPAGE_EXCEEDED);

    build_remove_liquidity(
        &env.provider,
        AMM_PROGRAM_ID,
        lp_amount,
        expected_a,
        expected_b,
        pool.remove_accounts(),
    )
    .send_and_confirm()
    .expect("a withdrawal returning exactly the minimums must succeed");
}

#[test]
fn remove_liquidity_rejects_more_than_the_lp_supply() {
    let env = Env::new(&TOKEN_PROGRAM_ID, &TOKEN_PROGRAM_ID);
    let (pool, _) = env.initialized();
    let result = build_remove_liquidity(
        &env.provider,
        AMM_PROGRAM_ID,
        SEED_LP + 1,
        1,
        1,
        pool.remove_accounts(),
    )
    .send_and_confirm();
    assert_custom_code(result, INSUFFICIENT_LIQUIDITY);
}

#[test]
fn remove_liquidity_rejects_a_zero_burn() {
    let env = Env::new(&TOKEN_PROGRAM_ID, &TOKEN_PROGRAM_ID);
    let (pool, _) = env.initialized();
    let result =
        build_remove_liquidity(&env.provider, AMM_PROGRAM_ID, 0, 0, 0, pool.remove_accounts())
            .send_and_confirm();
    assert_custom_code(result, ZERO_AMOUNT);
}

#[test]
fn swap_enforces_the_minimum_output() {
    let env = Env::new(&TOKEN_PROGRAM_ID, &TOKEN_PROGRAM_ID);
    let (pool, _) = env.initialized();
    let result = build_swap(
        &env.provider,
        AMM_PROGRAM_ID,
        100_000_000,
        u64::MAX,
        pool.swap_accounts(true),
    )
    .send_and_confirm();
    assert_custom_code(result, SLIPPAGE_EXCEEDED);
}

#[test]
fn swap_rejects_a_zero_input() {
    let env = Env::new(&TOKEN_PROGRAM_ID, &TOKEN_PROGRAM_ID);
    let (pool, _) = env.initialized();
    let result = build_swap(&env.provider, AMM_PROGRAM_ID, 0, 0, pool.swap_accounts(true))
        .send_and_confirm();
    assert_custom_code(result, ZERO_AMOUNT);
}

#[test]
fn swap_rejects_a_user_account_of_the_wrong_mint() {
    let env = Env::new(&TOKEN_PROGRAM_ID, &TOKEN_PROGRAM_ID);
    let (pool, _) = env.initialized();
    let mut accounts = pool.swap_accounts(true);
    accounts.user_source_token = pool.user_lp;
    let result = build_swap(&env.provider, AMM_PROGRAM_ID, 100_000_000, 1, accounts)
        .send_and_confirm();
    assert_custom_code(result, INVALID_MINT);
}
