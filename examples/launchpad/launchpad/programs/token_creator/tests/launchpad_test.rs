//! Token creator integration tests: a full launch for every pairing of the
//! quote mint's token program and address order against the new mint, what the
//! launched pool and the locked liquidity shares allow afterwards, and the
//! launch's rejection paths. Every launch uses an `id` for which each address
//! search in the transaction ends on its first attempt, so compute units are
//! the same from run to run.

use {
    amm_client::{
        get_pool_state_pda,
        instructions::{build_add_liquidity, build_swap, AddLiquidityAccounts, SwapAccounts},
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
    token_creator_client::{
        instructions::{build_launch_token, LaunchTokenAccounts},
        state::{fetch_launch_record, LaunchRecord},
        types::{LaunchTokenArgs, PROGRAM_ID as CREATOR_PROGRAM_ID},
    },
};

const OVERFLOW: u32 = 6000;
const AMM_ZERO_AMOUNT: u32 = 6002;
const TOKEN_PROGRAM_OWNER_MISMATCH: u32 = 4;
const ALREADY_INITIALIZED_AT_LAUNCH_RECORD: u32 = 10_214;
const ALREADY_INITIALIZED_AT_MINT: u32 = 10_314;
const WRONG_AUTHORITY_AT_AMM_DEPOSITOR_TOKEN_A: u32 = 10_736;
const PROGRAM_ID_MISMATCH_AT_AMM_TOKEN_A_MINT: u32 = 10_109;

const DECIMALS: u8 = 6;
const QUOTE_BALANCE: u64 = 10_000_000_000;
const AMOUNT_TOKEN_POOL: u64 = 1_000_000_000;
const AMOUNT_TOKEN_LAUNCHER: u64 = 500_000_000;
const AMOUNT_QUOTE: u64 = 2_000_000_000;

fn find(seeds: &[&[u8]], program: &Address) -> (Address, u8) {
    Address::find_program_address(seeds, program)
}

fn failure_code(result: Result<NaclacTransactionMetadata, NaclacClientError>) -> u32 {
    match result {
        Err(NaclacClientError::TransactionFailed {
            instruction_err: InstructionError::Custom(code),
            ..
        }) => code,
        other => panic!("expected a custom-code transaction failure, got {other:?}"),
    }
}

fn assert_custom_code(result: Result<NaclacTransactionMetadata, NaclacClientError>, expected: u32) {
    assert_eq!(failure_code(result), expected, "wrong custom error code");
}

/// Every address a launch of `id` by one payer uses, with the canonical bump
/// of each address that is searched for on chain: launch record, mint, the
/// payer's token account, pool, LP mint, both vaults and the payer's LP
/// account.
struct Derived {
    launch_record: Address,
    mint: Address,
    payer_token_account: Address,
    mint_is_pool_token_a: bool,
    pool_state: Address,
    lp_mint: Address,
    vault_a: Address,
    vault_b: Address,
    payer_lp: Address,
    bumps: [u8; 8],
}

fn derive(payer: &Address, quote_mint: &Address, id: u64) -> Derived {
    let id_bytes = id.to_le_bytes();
    let (launch_record, launch_record_bump) = find(
        &[&b"launch"[..], payer.as_ref(), &id_bytes],
        &CREATOR_PROGRAM_ID,
    );
    let (mint, mint_bump) = find(
        &[&b"mint"[..], payer.as_ref(), &id_bytes],
        &CREATOR_PROGRAM_ID,
    );
    let (payer_token_account, token_account_bump) = find(
        &[payer.as_ref(), TOKEN_PROGRAM_ID.as_ref(), mint.as_ref()],
        &ASSOCIATED_TOKEN_PROGRAM_ID,
    );
    let mint_is_pool_token_a = mint.to_bytes() < quote_mint.to_bytes();
    let (token_a, token_b) = if mint_is_pool_token_a {
        (mint, *quote_mint)
    } else {
        (*quote_mint, mint)
    };
    let (pool_state, pool_bump) = get_pool_state_pda(&AMM_PROGRAM_ID, &token_a, &token_b, id);
    let (lp_mint, lp_mint_bump) =
        find(&[&b"lp_mint"[..], pool_state.as_ref()], &AMM_PROGRAM_ID);
    let (vault_a, vault_a_bump) = find(
        &[&b"vault"[..], pool_state.as_ref(), token_a.as_ref()],
        &AMM_PROGRAM_ID,
    );
    let (vault_b, vault_b_bump) = find(
        &[&b"vault"[..], pool_state.as_ref(), token_b.as_ref()],
        &AMM_PROGRAM_ID,
    );
    let (payer_lp, payer_lp_bump) = find(
        &[payer.as_ref(), TOKEN_PROGRAM_ID.as_ref(), lp_mint.as_ref()],
        &ASSOCIATED_TOKEN_PROGRAM_ID,
    );
    Derived {
        launch_record,
        mint,
        payer_token_account,
        mint_is_pool_token_a,
        pool_state,
        lp_mint,
        vault_a,
        vault_b,
        payer_lp,
        bumps: [
            launch_record_bump,
            mint_bump,
            token_account_bump,
            pool_bump,
            lp_mint_bump,
            vault_a_bump,
            vault_b_bump,
            payer_lp_bump,
        ],
    }
}

struct Scenario {
    provider: NaclacProvider,
    id: u64,
    derived: Derived,
    quote_mint: Address,
    quote_program: Address,
    payer_quote_account: Address,
}

impl Scenario {
    /// A funded quote mint under `quote_program`, and the smallest launch `id`
    /// for which every address search ends at bump 255 and the quote mint
    /// sorts below the new mint exactly when `quote_below_mint` is set.
    fn new(quote_program: Address, quote_below_mint: bool) -> Scenario {
        let payer = load_node_wallet().expect("Failed to load local Solana keypair");
        let provider = NaclacProvider::new("litesvm", payer)
            .expect("Failed to construct NaclacProvider");
        let owner = provider.payer.address();

        let quote_signer = Keypair::new();
        let quote_mint = quote_signer.address();
        let (id, derived) = (0u64..)
            .map(|id| (id, derive(&owner, &quote_mint, id)))
            .find(|(_, derived)| {
                derived.mint_is_pool_token_a != quote_below_mint && derived.bumps == [255; 8]
            })
            .expect("a launch id with every bump at 255 exists");

        create_mint_with_program(&provider, &quote_signer, &owner, DECIMALS, &quote_program)
            .expect("Failed to create the quote mint");
        let payer_quote_account =
            create_ata_with_program(&provider, &quote_mint, &owner, &quote_program)
                .expect("Failed to create the payer's quote account");
        mint_to_with_program(
            &provider,
            &quote_mint,
            &payer_quote_account,
            &provider.payer,
            QUOTE_BALANCE,
            &quote_program,
        )
        .expect("Failed to mint quote tokens");
        Scenario { provider, id, derived, quote_mint, quote_program, payer_quote_account }
    }

    fn payer(&self) -> Address {
        self.provider.payer.address()
    }

    /// The pool vault that holds `mint`, which is one of the pool's two mints.
    fn vault_of(&self, mint: &Address) -> Address {
        let is_new_mint = *mint == self.derived.mint;
        if is_new_mint == self.derived.mint_is_pool_token_a {
            self.derived.vault_a
        } else {
            self.derived.vault_b
        }
    }

    /// The pool's two sides in the AMM's order, each as the mint, the payer's
    /// token account for it and the token program that owns the mint.
    fn pool_sides(&self) -> [(Address, Address, Address); 2] {
        let new_token = (
            self.derived.mint,
            self.derived.payer_token_account,
            TOKEN_PROGRAM_ID,
        );
        let quote = (self.quote_mint, self.payer_quote_account, self.quote_program);
        if self.derived.mint_is_pool_token_a {
            [new_token, quote]
        } else {
            [quote, new_token]
        }
    }

    fn accounts_for(
        &self,
        payer: Address,
        derived: &Derived,
        payer_quote_account: Address,
    ) -> LaunchTokenAccounts {
        LaunchTokenAccounts {
            payer,
            quote_mint: self.quote_mint,
            launch_record: derived.launch_record,
            mint: derived.mint,
            payer_token_account: derived.payer_token_account,
            payer_quote_account,
            pool_state: derived.pool_state,
            pool_vault_a: derived.vault_a,
            pool_vault_b: derived.vault_b,
            pool_lp_mint: derived.lp_mint,
            payer_lp: derived.payer_lp,
            token_program: TOKEN_PROGRAM_ID,
            quote_token_program: self.quote_program,
            associated_token_program: ASSOCIATED_TOKEN_PROGRAM_ID,
            system_program: SYSTEM_PROGRAM_ID,
            amm_program: AMM_PROGRAM_ID,
        }
    }

    fn accounts(&self) -> LaunchTokenAccounts {
        self.accounts_for(self.payer(), &self.derived, self.payer_quote_account)
    }

    fn launch_args(
        &self,
        amount_token_pool: u64,
        amount_token_launcher: u64,
        amount_quote: u64,
    ) -> LaunchTokenArgs {
        LaunchTokenArgs {
            id: self.id,
            decimals: DECIMALS,
            amount_token_pool,
            amount_token_launcher,
            amount_quote,
        }
    }

    fn launch_with(
        &self,
        accounts: LaunchTokenAccounts,
        amount_token_pool: u64,
        amount_token_launcher: u64,
        amount_quote: u64,
    ) -> Result<NaclacTransactionMetadata, NaclacClientError> {
        build_launch_token(
            &self.provider,
            CREATOR_PROGRAM_ID,
            self.launch_args(amount_token_pool, amount_token_launcher, amount_quote),
            accounts,
        )
        .send_and_confirm()
    }

    fn launch(
        &self,
        amount_token_pool: u64,
        amount_token_launcher: u64,
        amount_quote: u64,
    ) -> Result<NaclacTransactionMetadata, NaclacClientError> {
        self.launch_with(self.accounts(), amount_token_pool, amount_token_launcher, amount_quote)
    }

    fn launch_default(&self) -> NaclacTransactionMetadata {
        self.launch(AMOUNT_TOKEN_POOL, AMOUNT_TOKEN_LAUNCHER, AMOUNT_QUOTE)
            .expect("Failed to launch the token")
    }

    fn token_account(&self, address: &Address) -> TokenAccount {
        AccountFetcher::new(&self.provider)
            .fetch::<TokenAccount>(address)
            .expect("Failed to read token account")
    }

    fn balance(&self, address: &Address) -> u64 {
        self.token_account(address).amount()
    }

    fn mint_account(&self, address: &Address) -> Mint {
        AccountFetcher::new(&self.provider)
            .fetch::<Mint>(address)
            .expect("Failed to read mint")
    }

    /// A new, empty LP token account owned by the payer.
    fn new_lp_account(&self) -> Address {
        let signer = Keypair::new();
        create_token_account(&self.provider, &signer, &self.derived.lp_mint, &self.payer())
            .expect("Failed to create an LP token account");
        signer.address()
    }
}

fn full_launch(quote_program: Address, quote_below_mint: bool) {
    let scenario = Scenario::new(quote_program, quote_below_mint);
    let derived = &scenario.derived;
    let meta = scenario.launch_default();
    println!("   📊 Compute Units (CUs) consumed: {}", meta.compute_units_consumed);
    for log in &meta.logs {
        println!("      {log}");
    }

    let record: LaunchRecord = fetch_launch_record(&scenario.provider, &derived.launch_record)
        .expect("Failed to fetch the launch record");
    assert_eq!(record.creator, scenario.payer());
    assert_eq!(record.mint, derived.mint);
    let (recorded_token, recorded_quote) = (record.amount_token, record.amount_quote);
    assert_eq!(recorded_token, AMOUNT_TOKEN_POOL);
    assert_eq!(recorded_quote, AMOUNT_QUOTE);

    let token = scenario.mint_account(&derived.mint);
    assert_eq!(token.supply(), AMOUNT_TOKEN_POOL + AMOUNT_TOKEN_LAUNCHER);
    assert_eq!(token.decimals(), DECIMALS);
    assert_eq!(token.0[0..4], [0u8; 4], "the mint authority must be revoked");
    assert_eq!(token.0[46..50], [0u8; 4], "the mint must have no freeze authority");

    assert_eq!(scenario.balance(&derived.payer_token_account), AMOUNT_TOKEN_LAUNCHER);
    assert_eq!(scenario.balance(&scenario.vault_of(&derived.mint)), AMOUNT_TOKEN_POOL);
    assert_eq!(scenario.balance(&scenario.vault_of(&scenario.quote_mint)), AMOUNT_QUOTE);
    assert_eq!(
        scenario.balance(&scenario.payer_quote_account),
        QUOTE_BALANCE - AMOUNT_QUOTE
    );

    let expected_lp = ((AMOUNT_TOKEN_POOL as f64) * (AMOUNT_QUOTE as f64)).sqrt() as u64;
    let locked = scenario.token_account(&derived.payer_lp);
    assert_eq!(locked.amount(), expected_lp);
    assert_eq!(
        locked.owner(),
        derived.launch_record,
        "the first LP shares must be owned by the launch record"
    );
    assert_eq!(scenario.mint_account(&derived.lp_mint).supply(), expected_lp);
}

#[test]
fn launches_with_a_classic_quote_below_the_new_mint() {
    full_launch(TOKEN_PROGRAM_ID, true);
}

#[test]
fn launches_with_a_classic_quote_above_the_new_mint() {
    full_launch(TOKEN_PROGRAM_ID, false);
}

#[test]
fn launches_with_a_token_2022_quote_below_the_new_mint() {
    full_launch(TOKEN_2022_PROGRAM_ID, true);
}

#[test]
fn launches_with_a_token_2022_quote_above_the_new_mint() {
    full_launch(TOKEN_2022_PROGRAM_ID, false);
}

#[test]
fn launches_with_no_launcher_allocation() {
    let scenario = Scenario::new(TOKEN_PROGRAM_ID, true);
    scenario
        .launch(AMOUNT_TOKEN_POOL, 0, AMOUNT_QUOTE)
        .expect("a launch without a launcher allocation must succeed");
    let derived = &scenario.derived;
    assert_eq!(scenario.mint_account(&derived.mint).supply(), AMOUNT_TOKEN_POOL);
    assert_eq!(scenario.balance(&derived.payer_token_account), 0);
    assert_eq!(scenario.balance(&scenario.vault_of(&derived.mint)), AMOUNT_TOKEN_POOL);
}

#[test]
fn another_wallet_can_launch_the_same_id() {
    let scenario = Scenario::new(TOKEN_PROGRAM_ID, true);
    scenario.launch_default();

    let wallet = Keypair::new();
    let wallet_address = wallet.address();
    transfer_sol(&scenario.provider, &wallet_address, 10_000_000_000)
        .expect("Failed to fund the second wallet");
    let wallet_quote_account = create_ata_with_program(
        &scenario.provider,
        &scenario.quote_mint,
        &wallet_address,
        &TOKEN_PROGRAM_ID,
    )
    .expect("Failed to create the second wallet's quote account");
    mint_to_with_program(
        &scenario.provider,
        &scenario.quote_mint,
        &wallet_quote_account,
        &scenario.provider.payer,
        QUOTE_BALANCE,
        &TOKEN_PROGRAM_ID,
    )
    .expect("Failed to mint quote tokens to the second wallet");

    let wallet_derived = derive(&wallet_address, &scenario.quote_mint, scenario.id);
    assert_ne!(wallet_derived.mint, scenario.derived.mint);
    build_launch_token(
        &scenario.provider,
        CREATOR_PROGRAM_ID,
        scenario.launch_args(AMOUNT_TOKEN_POOL, AMOUNT_TOKEN_LAUNCHER, AMOUNT_QUOTE),
        scenario.accounts_for(wallet_address, &wallet_derived, wallet_quote_account),
    )
    .signer(&wallet)
    .send_and_confirm()
    .expect("a second wallet must be able to launch an id the first already used");

    let first = scenario.mint_account(&scenario.derived.mint);
    let second = scenario.mint_account(&wallet_derived.mint);
    assert_eq!(first.supply(), AMOUNT_TOKEN_POOL + AMOUNT_TOKEN_LAUNCHER);
    assert_eq!(second.supply(), AMOUNT_TOKEN_POOL + AMOUNT_TOKEN_LAUNCHER);
}

#[test]
fn the_creator_cannot_move_the_locked_liquidity_shares() {
    let scenario = Scenario::new(TOKEN_PROGRAM_ID, true);
    scenario.launch_default();
    let locked = scenario.derived.payer_lp;
    let destination = scenario.new_lp_account();

    let mut data = vec![3u8];
    data.extend_from_slice(&1u64.to_le_bytes());
    let result = InstructionBuilder::new(&scenario.provider, TOKEN_PROGRAM_ID, data)
        .account(
            AccountMeta { address: locked, is_signer: false, is_writable: true },
            "source",
        )
        .account(
            AccountMeta { address: destination, is_signer: false, is_writable: true },
            "destination",
        )
        .account(
            AccountMeta { address: scenario.payer(), is_signer: true, is_writable: false },
            "authority",
        )
        .send_and_confirm();
    assert_custom_code(result, TOKEN_PROGRAM_OWNER_MISMATCH);
    assert_eq!(scenario.balance(&destination), 0);
}

#[test]
fn liquidity_can_still_be_added_after_the_launch() {
    let scenario = Scenario::new(TOKEN_PROGRAM_ID, true);
    scenario.launch_default();
    let derived = &scenario.derived;
    let locked_before = scenario.balance(&derived.payer_lp);
    let supply_before = scenario.mint_account(&derived.lp_mint).supply();
    let new_lp = scenario.new_lp_account();

    let [side_a, side_b] = scenario.pool_sides();
    let max_amount = |side: &(Address, Address, Address)| {
        if side.0 == derived.mint {
            100_000_000
        } else {
            200_000_000
        }
    };
    build_add_liquidity(
        &scenario.provider,
        AMM_PROGRAM_ID,
        max_amount(&side_a),
        max_amount(&side_b),
        1,
        AddLiquidityAccounts {
            user: scenario.payer(),
            pool_state: derived.pool_state,
            token_a_mint: side_a.0,
            token_b_mint: side_b.0,
            vault_a: derived.vault_a,
            vault_b: derived.vault_b,
            lp_mint: derived.lp_mint,
            user_token_a: side_a.1,
            user_token_b: side_b.1,
            user_lp: new_lp,
            token_program_a: side_a.2,
            token_program_b: side_b.2,
            lp_token_program: TOKEN_PROGRAM_ID,
        },
    )
    .send_and_confirm()
    .expect("adding liquidity to a launched pool must succeed");

    let minted = scenario.balance(&new_lp);
    assert_eq!(minted, 141_421_356);
    assert_eq!(scenario.mint_account(&derived.lp_mint).supply(), supply_before + minted);
    assert_eq!(scenario.balance(&derived.payer_lp), locked_before);
    assert_eq!(scenario.token_account(&derived.payer_lp).owner(), derived.launch_record);
}

#[test]
fn the_launched_pool_can_be_swapped() {
    let scenario = Scenario::new(TOKEN_PROGRAM_ID, false);
    scenario.launch_default();
    let derived = &scenario.derived;
    let swap_in = 10_000_000;
    let token_before = scenario.balance(&derived.payer_token_account);
    let quote_before = scenario.balance(&scenario.payer_quote_account);

    let [side_a, side_b] = scenario.pool_sides();
    build_swap(
        &scenario.provider,
        AMM_PROGRAM_ID,
        swap_in,
        1,
        SwapAccounts {
            user: scenario.payer(),
            pool_state: derived.pool_state,
            token_a_mint: side_a.0,
            token_b_mint: side_b.0,
            pool_source_vault: scenario.vault_of(&derived.mint),
            pool_destination_vault: scenario.vault_of(&scenario.quote_mint),
            user_source_token: derived.payer_token_account,
            user_destination_token: scenario.payer_quote_account,
            token_program_a: side_a.2,
            token_program_b: side_b.2,
        },
    )
    .send_and_confirm()
    .expect("swapping against a launched pool must succeed");

    assert_eq!(scenario.balance(&derived.payer_token_account), token_before - swap_in);
    assert!(scenario.balance(&scenario.payer_quote_account) > quote_before);
}

#[test]
fn rejects_launching_the_same_id_twice() {
    let scenario = Scenario::new(TOKEN_PROGRAM_ID, true);
    scenario.launch_default();
    let code = failure_code(scenario.launch(
        AMOUNT_TOKEN_POOL,
        AMOUNT_TOKEN_LAUNCHER + 1,
        AMOUNT_QUOTE,
    ));
    assert!(
        code == ALREADY_INITIALIZED_AT_LAUNCH_RECORD || code == ALREADY_INITIALIZED_AT_MINT,
        "a second launch of the same id must fail as already initialized, got {code}"
    );
}

#[test]
fn rejects_token_amounts_that_overflow() {
    let scenario = Scenario::new(TOKEN_PROGRAM_ID, true);
    let result = scenario.launch(u64::MAX, 1, AMOUNT_QUOTE);
    assert_custom_code(result, OVERFLOW);
}

#[test]
fn rejects_a_zero_quote_amount() {
    let scenario = Scenario::new(TOKEN_PROGRAM_ID, false);
    let result = scenario.launch(AMOUNT_TOKEN_POOL, AMOUNT_TOKEN_LAUNCHER, 0);
    assert_custom_code(result, AMM_ZERO_AMOUNT);
}

#[test]
fn rejects_a_zero_pool_token_amount() {
    let scenario = Scenario::new(TOKEN_PROGRAM_ID, true);
    let result = scenario.launch(0, AMOUNT_TOKEN_LAUNCHER, AMOUNT_QUOTE);
    assert_custom_code(result, AMM_ZERO_AMOUNT);
}

#[test]
fn rejects_a_token_program_that_does_not_own_the_quote_mint() {
    let scenario = Scenario::new(TOKEN_2022_PROGRAM_ID, true);
    let mut accounts = scenario.accounts();
    accounts.quote_token_program = TOKEN_PROGRAM_ID;
    let result = scenario.launch_with(accounts, AMOUNT_TOKEN_POOL, AMOUNT_TOKEN_LAUNCHER, AMOUNT_QUOTE);
    assert_custom_code(result, PROGRAM_ID_MISMATCH_AT_AMM_TOKEN_A_MINT);
}

#[test]
fn rejects_a_quote_account_owned_by_someone_else() {
    let scenario = Scenario::new(TOKEN_PROGRAM_ID, true);
    let stranger = Keypair::new().address();
    let account_signer = Keypair::new();
    create_token_account(&scenario.provider, &account_signer, &scenario.quote_mint, &stranger)
        .expect("Failed to create the stranger's quote account");
    mint_to_with_program(
        &scenario.provider,
        &scenario.quote_mint,
        &account_signer.address(),
        &scenario.provider.payer,
        QUOTE_BALANCE,
        &TOKEN_PROGRAM_ID,
    )
    .expect("Failed to mint quote tokens to the stranger's account");

    let mut accounts = scenario.accounts();
    accounts.payer_quote_account = account_signer.address();
    let result = scenario.launch_with(accounts, AMOUNT_TOKEN_POOL, AMOUNT_TOKEN_LAUNCHER, AMOUNT_QUOTE);
    assert_custom_code(result, WRONG_AUTHORITY_AT_AMM_DEPOSITOR_TOKEN_A);
}
