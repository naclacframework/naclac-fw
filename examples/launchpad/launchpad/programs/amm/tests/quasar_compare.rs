//! Runs the same pool lifecycle against the naclac AMM and the Quasar AMM
//! (`quasar-master/examples/amm`) in one litesvm harness, checks that both end
//! in the same balances, and prints compute units per instruction and the
//! size of each `.so`.
//!
//! The Quasar binary is read from the path in `QUASAR_AMM_SO`; the naclac
//! binary is the one `naclac build -p amm` leaves in `artifacts/deploy`.

use {
    amm_client::{
        get_pool_state_pda,
        instructions::{
            build_add_liquidity, build_initialize, build_remove_liquidity, build_swap,
            AddLiquidityAccounts, InitializeAccounts, RemoveLiquidityAccounts, SwapAccounts,
        },
        types::PROGRAM_ID as NACLAC_AMM_ID,
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

const QUASAR_AMM_ID: Address = address!("55555555555555555555555555555555555555555555");

const USER_BALANCE: u64 = 10_000_000_000;
const SEED_AMOUNT_A: u64 = 2_000_000_000;
const SEED_AMOUNT_B: u64 = 4_000_000_000;
const ADD_AMOUNT_A: u64 = 500_000_000;
const ADD_AMOUNT_B: u64 = 1_000_000_000;
const SWAP_IN: u64 = 100_000_000;
const REMOVE_LP: u64 = 500_000_000;

#[derive(Clone, Copy, PartialEq)]
enum Framework {
    Naclac,
    Quasar,
}

impl Framework {
    fn name(self) -> &'static str {
        match self {
            Framework::Naclac => "naclac",
            Framework::Quasar => "quasar",
        }
    }

    fn program_id(self) -> Address {
        match self {
            Framework::Naclac => NACLAC_AMM_ID,
            Framework::Quasar => QUASAR_AMM_ID,
        }
    }
}

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

#[derive(Debug, PartialEq)]
struct Outcome {
    vault_a: u64,
    vault_b: u64,
    user_a: u64,
    user_b: u64,
    user_lp: u64,
}

struct Report {
    initialize: NaclacTransactionMetadata,
    add_liquidity: NaclacTransactionMetadata,
    swap_a_for_b: NaclacTransactionMetadata,
    swap_b_for_a: NaclacTransactionMetadata,
    remove_liquidity: NaclacTransactionMetadata,
    outcome: Outcome,
}

/// One top-level program's compute units split at its direct cross-program
/// calls, read from the runtime's own `consumed N of R compute units` log
/// lines. `R` is the budget left when a call starts, so the gap between two
/// consecutive calls (and before the first / after the last) is what the
/// program itself spent there, plus the runtime's fixed cost of making the
/// call.
struct Trace {
    total: u64,
    gaps: Vec<u64>,
    children: Vec<(String, u64)>,
}

fn consumed_of(line: &str) -> Option<(u64, u64)> {
    let rest = line.split(" consumed ").nth(1)?;
    let mut parts = rest.split(" of ");
    let consumed = parts.next()?.trim().parse().ok()?;
    let remaining = parts.next()?.split(' ').next()?.parse().ok()?;
    Some((consumed, remaining))
}

fn trace(program: &Address, logs: &[String]) -> Trace {
    let program = program.to_string();
    let mut stack: Vec<(String, usize)> = Vec::new();
    let mut top_level: Option<(u64, u64)> = None;
    let mut children: Vec<(String, u64, u64, usize)> = Vec::new();
    let mut pending: Vec<Option<(u64, u64)>> = Vec::new();
    let mut builtins_since_last = 0usize;
    for line in logs {
        let Some(rest) = line.strip_prefix("Program ") else { continue };
        let id = rest.split(' ').next().unwrap_or_default().to_string();
        if let Some(depth) = rest
            .split(" invoke [")
            .nth(1)
            .and_then(|d| d.strip_suffix(']'))
            .and_then(|d| d.parse::<usize>().ok())
        {
            stack.push((id, depth));
            pending.push(None);
        } else if let Some(pair) = consumed_of(line) {
            if let Some(slot) = pending.last_mut() {
                *slot = Some(pair);
            }
        } else if rest.ends_with(" success") || rest.contains(" failed") {
            let (popped, depth) = stack.pop().expect("log stack underflow");
            let pair = pending.pop().flatten();
            if depth == 1 && popped == program {
                top_level = pair;
            } else if depth == 2 {
                match pair {
                    Some((consumed, remaining)) => {
                        children.push((popped, consumed, remaining, builtins_since_last));
                        builtins_since_last = 0;
                    }
                    None => builtins_since_last += 1,
                }
            }
        }
    }
    let (total, start) = top_level.expect("no top-level consumed line");
    let mut gaps = Vec::new();
    let mut cursor = start;
    for (_, consumed, remaining, _) in &children {
        gaps.push(cursor - remaining);
        cursor = remaining - consumed;
    }
    gaps.push(cursor - (start - total));
    Trace {
        total,
        gaps,
        children: children
            .into_iter()
            .map(|(id, consumed, _, builtins)| {
                let short: String = id.chars().take(8).collect();
                (format!("{short}+{builtins}sys"), consumed)
            })
            .collect(),
    }
}

fn print_trace(label: &str, naclac: &NaclacTransactionMetadata, quasar: &NaclacTransactionMetadata) {
    print_trace_logs(label, &naclac.logs, &quasar.logs);
}

fn logs_of(result: &Result<NaclacTransactionMetadata, NaclacClientError>) -> Vec<String> {
    match result {
        Ok(meta) => meta.logs.clone(),
        Err(NaclacClientError::TransactionFailed { logs, .. }) => logs.clone(),
        Err(other) => panic!("unexpected client error: {other:?}"),
    }
}

fn print_trace_logs(label: &str, naclac: &[String], quasar: &[String]) {
    let n = trace(&NACLAC_AMM_ID, naclac);
    let q = trace(&QUASAR_AMM_ID, quasar);
    println!("   -- {label}: program-only CU {} vs {} --", n.total, q.total);
    let same_calls = n.children.len() == q.children.len()
        && n.children.iter().zip(&q.children).all(|(a, b)| a.0 == b.0);
    println!("   same call sequence: {same_calls}");
    if !same_calls {
        println!("   naclac calls {:?}", n.children);
        println!("   quasar calls {:?}", q.children);
        return;
    }
    println!(
        "   {:<26} {:>8} {:>8} {:>7}   {:>6} {:>6}",
        "segment (ends at call)", "naclac", "quasar", "delta", "ch.n", "ch.q"
    );
    for (i, (id, nc)) in n.children.iter().enumerate() {
        let qc = q.children[i].1;
        println!(
            "   {:<26} {:>8} {:>8} {:>+7}   {:>6} {:>6}",
            format!("#{i} before {id}"),
            n.gaps[i],
            q.gaps[i],
            i128::from(q.gaps[i]) - i128::from(n.gaps[i]),
            nc,
            qc
        );
    }
    let last = n.gaps.len() - 1;
    println!(
        "   {:<26} {:>8} {:>8} {:>+7}",
        "tail after last call",
        n.gaps[last],
        q.gaps[last],
        i128::from(q.gaps[last]) - i128::from(n.gaps[last])
    );
}

struct Env {
    framework: Framework,
    provider: NaclacProvider,
    lo: TestMint,
    hi: TestMint,
}

fn quasar_so_path() -> String {
    std::env::var("QUASAR_AMM_SO").unwrap_or_else(|_| {
        panic!(
            "QUASAR_AMM_SO is not set; point it at the built Quasar AMM binary, for example \
             QUASAR_AMM_SO=/path/to/quasar-master/target/deploy/quasar_amm.so"
        )
    })
}

fn naclac_so_path() -> String {
    format!(
        "{}/../../artifacts/deploy/amm.so",
        env!("CARGO_MANIFEST_DIR")
    )
}

fn file_size(path: &str) -> u64 {
    std::fs::metadata(path)
        .unwrap_or_else(|e| panic!("cannot read {path}: {e}"))
        .len()
}

fn sorted_signers() -> (Keypair, Keypair) {
    let (x, y) = (Keypair::new(), Keypair::new());
    if x.address().to_bytes() < y.address().to_bytes() {
        (x, y)
    } else {
        (y, x)
    }
}

fn standard_mint(provider: &NaclacProvider, signer: &Keypair, program: &Address) -> TestMint {
    create_mint_with_program(provider, signer, &provider.payer.address(), 6, program)
        .expect("Failed to create mint");
    let address = signer.address();
    let user_account =
        create_ata_with_program(provider, &address, &provider.payer.address(), program)
            .expect("Failed to create user token account");
    mint_to_with_program(
        provider,
        &address,
        &user_account,
        &provider.payer,
        USER_BALANCE,
        program,
    )
    .expect("Failed to mint to user");
    TestMint { address, program: *program, user_account }
}

impl Env {
    fn new(framework: Framework, program_lo: &Address, program_hi: &Address) -> Env {
        let payer = load_node_wallet().expect("Failed to load local Solana keypair");
        let provider =
            NaclacProvider::new("litesvm", payer).expect("Failed to construct NaclacProvider");
        if framework == Framework::Quasar {
            provider
                .add_program(&QUASAR_AMM_ID, &quasar_so_path())
                .expect("Failed to load the Quasar AMM binary");
        }
        let (lo_signer, hi_signer) = sorted_signers();
        let lo = standard_mint(&provider, &lo_signer, program_lo);
        let hi = standard_mint(&provider, &hi_signer, program_hi);
        Env { framework, provider, lo, hi }
    }

    fn derive(&self, id: u64) -> (Pool, [u8; 5]) {
        let program = self.framework.program_id();
        let user = self.provider.payer.address();
        let (pool_state, pool_bump) = match self.framework {
            Framework::Naclac => {
                get_pool_state_pda(&program, &self.lo.address, &self.hi.address, id)
            }
            Framework::Quasar => Address::find_program_address(
                &[
                    &b"pool"[..],
                    self.lo.address.as_ref(),
                    self.hi.address.as_ref(),
                    &id.to_le_bytes(),
                ],
                &program,
            ),
        };
        let (lp_mint, lp_mint_bump) =
            Address::find_program_address(&[&b"lp_mint"[..], pool_state.as_ref()], &program);
        let vault = |mint: &Address| {
            Address::find_program_address(
                &[&b"vault"[..], pool_state.as_ref(), mint.as_ref()],
                &program,
            )
        };
        let (vault_a, vault_a_bump) = vault(&self.lo.address);
        let (vault_b, vault_b_bump) = vault(&self.hi.address);
        let (user_lp, user_lp_bump) = Address::find_program_address(
            &[user.as_ref(), TOKEN_PROGRAM_ID.as_ref(), lp_mint.as_ref()],
            &ASSOCIATED_TOKEN_PROGRAM_ID,
        );
        let pool = Pool {
            user,
            token_a_mint: self.lo.address,
            token_b_mint: self.hi.address,
            token_program_a: self.lo.program,
            token_program_b: self.hi.program,
            pool_state,
            lp_mint,
            vault_a,
            vault_b,
            user_token_a: self.lo.user_account,
            user_token_b: self.hi.user_account,
            user_lp,
        };
        (pool, [pool_bump, lp_mint_bump, vault_a_bump, vault_b_bump, user_lp_bump])
    }

    fn grounded_id(&self) -> u64 {
        (0u64..)
            .find(|&id| self.derive(id).1 == [255; 5])
            .expect("a pool number with every bump at 255 exists")
    }

    fn ungrounded_id(&self) -> u64 {
        (0u64..)
            .find(|&id| {
                let bumps = self.derive(id).1;
                bumps[0] != 255 && bumps[4] != 255
            })
            .expect("a pool number with the pool and LP account below 255 exists")
    }

    fn balance(&self, address: &Address) -> u64 {
        AccountFetcher::new(&self.provider)
            .fetch::<TokenAccount>(address)
            .expect("Failed to read token account")
            .amount()
    }

    fn try_raw(
        &self,
        discriminator: u8,
        args: &[u64],
        accounts: Vec<(AccountMeta, &'static str)>,
    ) -> Result<NaclacTransactionMetadata, NaclacClientError> {
        let mut data = vec![discriminator, 0, 0, 0, 0, 0, 0, 0];
        for arg in args {
            data.extend_from_slice(&arg.to_le_bytes());
        }
        InstructionBuilder::new(&self.provider, QUASAR_AMM_ID, data)
            .accounts(accounts)
            .send_and_confirm()
    }

    fn initialize(
        &self,
        pool: &Pool,
        id: u64,
        amount_a: u64,
        amount_b: u64,
    ) -> Result<NaclacTransactionMetadata, NaclacClientError> {
        match self.framework {
            Framework::Naclac => {
                build_initialize(
                    &self.provider,
                    NACLAC_AMM_ID,
                    id,
                    amount_a,
                    amount_b,
                    InitializeAccounts {
                        payer: pool.user,
                        token_a_mint: pool.token_a_mint,
                        token_b_mint: pool.token_b_mint,
                        pool_state: pool.pool_state,
                        lp_mint: pool.lp_mint,
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
                .send_and_confirm()
            }
            Framework::Quasar => self.try_raw(
                1,
                &[id, amount_a, amount_b],
                vec![
                    (AccountMeta::new(pool.user, true), "payer"),
                    (AccountMeta::new_readonly(pool.token_a_mint, false), "token_a_mint"),
                    (AccountMeta::new_readonly(pool.token_b_mint, false), "token_b_mint"),
                    (AccountMeta::new(pool.pool_state, false), "pool_state"),
                    (AccountMeta::new(pool.lp_mint, false), "lp_mint"),
                    (AccountMeta::new(pool.vault_a, false), "vault_a"),
                    (AccountMeta::new(pool.vault_b, false), "vault_b"),
                    (AccountMeta::new(pool.user_token_a, false), "depositor_token_a"),
                    (AccountMeta::new(pool.user_token_b, false), "depositor_token_b"),
                    (AccountMeta::new(pool.user_lp, false), "depositor_lp"),
                    (AccountMeta::new_readonly(pool.token_program_a, false), "token_program_a"),
                    (AccountMeta::new_readonly(pool.token_program_b, false), "token_program_b"),
                    (AccountMeta::new_readonly(TOKEN_PROGRAM_ID, false), "lp_token_program"),
                    (
                        AccountMeta::new_readonly(ASSOCIATED_TOKEN_PROGRAM_ID, false),
                        "associated_token_program",
                    ),
                    (AccountMeta::new_readonly(SYSTEM_PROGRAM_ID, false), "system_program"),
                ],
            ),
        }
    }

    fn pool_accounts(&self, pool: &Pool) -> Vec<(AccountMeta, &'static str)> {
        vec![
            (AccountMeta::new(pool.user, true), "user"),
            (AccountMeta::new(pool.pool_state, false), "pool_state"),
            (AccountMeta::new_readonly(pool.token_a_mint, false), "token_a_mint"),
            (AccountMeta::new_readonly(pool.token_b_mint, false), "token_b_mint"),
            (AccountMeta::new(pool.vault_a, false), "vault_a"),
            (AccountMeta::new(pool.vault_b, false), "vault_b"),
            (AccountMeta::new(pool.lp_mint, false), "lp_mint"),
            (AccountMeta::new(pool.user_token_a, false), "user_token_a"),
            (AccountMeta::new(pool.user_token_b, false), "user_token_b"),
            (AccountMeta::new(pool.user_lp, false), "user_lp"),
            (AccountMeta::new_readonly(pool.token_program_a, false), "token_program_a"),
            (AccountMeta::new_readonly(pool.token_program_b, false), "token_program_b"),
            (AccountMeta::new_readonly(TOKEN_PROGRAM_ID, false), "lp_token_program"),
        ]
    }

    fn add_liquidity(
        &self,
        pool: &Pool,
        max_a: u64,
        max_b: u64,
        min_lp: u64,
    ) -> Result<NaclacTransactionMetadata, NaclacClientError> {
        match self.framework {
            Framework::Naclac => {
                build_add_liquidity(
                    &self.provider,
                    NACLAC_AMM_ID,
                    max_a,
                    max_b,
                    min_lp,
                    AddLiquidityAccounts {
                        user: pool.user,
                        pool_state: pool.pool_state,
                        token_a_mint: pool.token_a_mint,
                        token_b_mint: pool.token_b_mint,
                        vault_a: pool.vault_a,
                        vault_b: pool.vault_b,
                        lp_mint: pool.lp_mint,
                        user_token_a: pool.user_token_a,
                        user_token_b: pool.user_token_b,
                        user_lp: pool.user_lp,
                        token_program_a: pool.token_program_a,
                        token_program_b: pool.token_program_b,
                        lp_token_program: TOKEN_PROGRAM_ID,
                    },
                )
                .send_and_confirm()
            }
            Framework::Quasar => {
                self.try_raw(3, &[max_a, max_b, min_lp], self.pool_accounts(pool))
            }
        }
    }

    fn swap(
        &self,
        pool: &Pool,
        a_for_b: bool,
        amount_in: u64,
        minimum_out: u64,
    ) -> Result<NaclacTransactionMetadata, NaclacClientError> {
        let (source_vault, destination_vault, source_token, destination_token) = if a_for_b {
            (pool.vault_a, pool.vault_b, pool.user_token_a, pool.user_token_b)
        } else {
            (pool.vault_b, pool.vault_a, pool.user_token_b, pool.user_token_a)
        };
        match self.framework {
            Framework::Naclac => {
                build_swap(
                    &self.provider,
                    NACLAC_AMM_ID,
                    amount_in,
                    minimum_out,
                    SwapAccounts {
                        user: pool.user,
                        pool_state: pool.pool_state,
                        token_a_mint: pool.token_a_mint,
                        token_b_mint: pool.token_b_mint,
                        pool_source_vault: source_vault,
                        pool_destination_vault: destination_vault,
                        user_source_token: source_token,
                        user_destination_token: destination_token,
                        token_program_a: pool.token_program_a,
                        token_program_b: pool.token_program_b,
                    },
                )
                .send_and_confirm()
            }
            Framework::Quasar => self.try_raw(
                2,
                &[amount_in, minimum_out],
                vec![
                    (AccountMeta::new(pool.user, true), "user"),
                    (AccountMeta::new_readonly(pool.pool_state, false), "pool_state"),
                    (AccountMeta::new_readonly(pool.token_a_mint, false), "token_a_mint"),
                    (AccountMeta::new_readonly(pool.token_b_mint, false), "token_b_mint"),
                    (AccountMeta::new(source_vault, false), "pool_source_vault"),
                    (AccountMeta::new(destination_vault, false), "pool_destination_vault"),
                    (AccountMeta::new(source_token, false), "user_source_token"),
                    (AccountMeta::new(destination_token, false), "user_destination_token"),
                    (AccountMeta::new_readonly(pool.token_program_a, false), "token_program_a"),
                    (AccountMeta::new_readonly(pool.token_program_b, false), "token_program_b"),
                ],
            ),
        }
    }

    fn remove_liquidity(
        &self,
        pool: &Pool,
        lp_amount: u64,
        min_a: u64,
        min_b: u64,
    ) -> Result<NaclacTransactionMetadata, NaclacClientError> {
        match self.framework {
            Framework::Naclac => {
                build_remove_liquidity(
                    &self.provider,
                    NACLAC_AMM_ID,
                    lp_amount,
                    min_a,
                    min_b,
                    RemoveLiquidityAccounts {
                        user: pool.user,
                        pool_state: pool.pool_state,
                        token_a_mint: pool.token_a_mint,
                        token_b_mint: pool.token_b_mint,
                        vault_a: pool.vault_a,
                        vault_b: pool.vault_b,
                        lp_mint: pool.lp_mint,
                        user_token_a: pool.user_token_a,
                        user_token_b: pool.user_token_b,
                        user_lp: pool.user_lp,
                        token_program_a: pool.token_program_a,
                        token_program_b: pool.token_program_b,
                        lp_token_program: TOKEN_PROGRAM_ID,
                    },
                )
                .send_and_confirm()
            }
            Framework::Quasar => {
                self.try_raw(4, &[lp_amount, min_a, min_b], self.pool_accounts(pool))
            }
        }
    }

    fn run(&self, id: u64) -> Report {
        let (pool, _) = self.derive(id);
        let initialize = self
            .initialize(&pool, id, SEED_AMOUNT_A, SEED_AMOUNT_B)
            .expect("initialize failed");
        let add_liquidity = self
            .add_liquidity(&pool, ADD_AMOUNT_A, ADD_AMOUNT_B, 707_106_781)
            .expect("add_liquidity failed");
        let swap_a_for_b = self
            .swap(&pool, true, SWAP_IN, 150_000_000)
            .expect("swap a->b failed");
        let swap_b_for_a = self
            .swap(&pool, false, SWAP_IN, 1)
            .expect("swap b->a failed");
        let remove_liquidity = self
            .remove_liquidity(&pool, REMOVE_LP, 1, 1)
            .expect("remove_liquidity failed");
        Report {
            initialize,
            add_liquidity,
            swap_a_for_b,
            swap_b_for_a,
            remove_liquidity,
            outcome: Outcome {
                vault_a: self.balance(&pool.vault_a),
                vault_b: self.balance(&pool.vault_b),
                user_a: self.balance(&pool.user_token_a),
                user_b: self.balance(&pool.user_token_b),
                user_lp: self.balance(&pool.user_lp),
            },
        }
    }
}

fn print_row(label: &str, naclac: &NaclacTransactionMetadata, quasar: &NaclacTransactionMetadata) {
    let (n, q) = (naclac.compute_units_consumed, quasar.compute_units_consumed);
    let delta = i128::from(q) - i128::from(n);
    println!("   {label:<18} {n:>8} {q:>8} {delta:>+8}");
}

fn compare(label: &str, program_lo: &Address, program_hi: &Address, grind: bool, detail: bool) {
    let run = |framework: Framework| {
        let env = Env::new(framework, program_lo, program_hi);
        let id = if grind { env.grounded_id() } else { env.ungrounded_id() };
        let report = env.run(id);
        println!("   {} pool number {id}", framework.name());
        report
    };
    println!("\n== {label} ==");
    let naclac = run(Framework::Naclac);
    let quasar = run(Framework::Quasar);

    assert_eq!(
        naclac.outcome, quasar.outcome,
        "both programs must leave the same balances"
    );

    println!("   {:<18} {:>8} {:>8} {:>8}", "instruction", "naclac", "quasar", "delta");
    print_row("initialize", &naclac.initialize, &quasar.initialize);
    print_row("add_liquidity", &naclac.add_liquidity, &quasar.add_liquidity);
    print_row("swap a->b", &naclac.swap_a_for_b, &quasar.swap_a_for_b);
    print_row("swap b->a", &naclac.swap_b_for_a, &quasar.swap_b_for_a);
    print_row("remove_liquidity", &naclac.remove_liquidity, &quasar.remove_liquidity);

    if detail {
        print_trace("initialize", &naclac.initialize, &quasar.initialize);
        print_trace("add_liquidity", &naclac.add_liquidity, &quasar.add_liquidity);
        print_trace("swap a->b", &naclac.swap_a_for_b, &quasar.swap_a_for_b);
        print_trace("remove_liquidity", &naclac.remove_liquidity, &quasar.remove_liquidity);
    }
}

#[test]
#[ignore = "needs QUASAR_AMM_SO; run with --ignored"]
fn compare_naclac_and_quasar_amm() {
    let naclac_size = file_size(&naclac_so_path());
    let quasar_size = file_size(&quasar_so_path());
    println!("\n== program size (bytes) ==");
    println!("   naclac {naclac_size}   quasar {quasar_size}");

    compare("Token + Token, every bump 255", &TOKEN_PROGRAM_ID, &TOKEN_PROGRAM_ID, true, true);
    compare(
        "Token + Token, pool and LP account below 255",
        &TOKEN_PROGRAM_ID,
        &TOKEN_PROGRAM_ID,
        false,
        false,
    );
    compare(
        "Token-2022 + Token-2022, every bump 255",
        &TOKEN_2022_PROGRAM_ID,
        &TOKEN_2022_PROGRAM_ID,
        true,
        false,
    );
    compare(
        "Token + Token-2022, every bump 255",
        &TOKEN_PROGRAM_ID,
        &TOKEN_2022_PROGRAM_ID,
        true,
        true,
    );
}

/// Each probe is a real transaction that the program rejects at a known
/// point, so the compute units it reports are the program's cost up to that
/// point. Rejected transactions leave no state behind, so the probes run on
/// one pool, and the full instruction runs last.
fn probe_cu(
    framework: Framework,
    result: &Result<NaclacTransactionMetadata, NaclacClientError>,
) -> u64 {
    trace(&framework.program_id(), &logs_of(result)).total
}

fn first_gap(framework: Framework, result: &Result<NaclacTransactionMetadata, NaclacClientError>) -> u64 {
    trace(&framework.program_id(), &logs_of(result)).gaps[0]
}

struct Probes {
    swap_zero_amount: u64,
    swap_slippage: u64,
    swap_first_gap: u64,
    add_zero_shares: u64,
    add_slippage: u64,
    add_first_gap: u64,
    remove_zero_lp: u64,
    remove_slippage: u64,
    remove_first_gap: u64,
    init_zero_amount_logs: Vec<String>,
    swap_zero_amount_logs: Vec<String>,
}

fn probes(framework: Framework) -> Probes {
    let env = Env::new(framework, &TOKEN_PROGRAM_ID, &TOKEN_PROGRAM_ID);
    let id = env.grounded_id();
    let (pool, _) = env.derive(id);

    let init_zero = env.initialize(&pool, id, 0, SEED_AMOUNT_B);
    let init_zero_amount_logs = logs_of(&init_zero);
    env.initialize(&pool, id, SEED_AMOUNT_A, SEED_AMOUNT_B)
        .expect("initialize failed");

    let swap_zero = env.swap(&pool, true, 0, 0);
    let swap_zero_amount_logs = logs_of(&swap_zero);
    let swap_slippage = env.swap(&pool, true, SWAP_IN, u64::MAX);
    let add_zero = env.add_liquidity(&pool, 0, 0, 0);
    let add_slippage = env.add_liquidity(&pool, ADD_AMOUNT_A, ADD_AMOUNT_B, u64::MAX);
    let remove_zero = env.remove_liquidity(&pool, 0, 1, 1);
    let remove_slippage = env.remove_liquidity(&pool, REMOVE_LP, u64::MAX, u64::MAX);

    let add_full = env.add_liquidity(&pool, ADD_AMOUNT_A, ADD_AMOUNT_B, 707_106_781);
    let swap_full = env.swap(&pool, true, SWAP_IN, 150_000_000);
    let remove_full = env.remove_liquidity(&pool, REMOVE_LP, 1, 1);
    assert!(
        add_full.is_ok() && swap_full.is_ok() && remove_full.is_ok(),
        "the full runs must succeed, or their first gap is not a first gap"
    );

    Probes {
        swap_zero_amount: probe_cu(framework, &swap_zero),
        swap_slippage: probe_cu(framework, &swap_slippage),
        swap_first_gap: first_gap(framework, &swap_full),
        add_zero_shares: probe_cu(framework, &add_zero),
        add_slippage: probe_cu(framework, &add_slippage),
        add_first_gap: first_gap(framework, &add_full),
        remove_zero_lp: probe_cu(framework, &remove_zero),
        remove_slippage: probe_cu(framework, &remove_slippage),
        remove_first_gap: first_gap(framework, &remove_full),
        init_zero_amount_logs,
        swap_zero_amount_logs,
    }
}

fn probe_row(label: &str, naclac: u64, quasar: u64) {
    println!(
        "   {label:<46} {naclac:>8} {quasar:>8} {:>+7}",
        i128::from(quasar) - i128::from(naclac)
    );
}

fn probe_section(
    title: &str,
    first: (&str, u64, u64),
    second: (&str, u64, u64),
    third: (&str, u64, u64),
) {
    println!("\n   {title}");
    println!("   {:<46} {:>8} {:>8} {:>7}", "stage (cumulative CU)", "naclac", "quasar", "delta");
    probe_row(first.0, first.1, first.2);
    probe_row(second.0, second.1, second.2);
    probe_row(third.0, third.1, third.2);
    probe_row(
        "  step 1 -> 2",
        second.1.saturating_sub(first.1),
        second.2.saturating_sub(first.2),
    );
    probe_row(
        "  step 2 -> 3",
        third.1.saturating_sub(second.1),
        third.2.saturating_sub(second.2),
    );
}

#[test]
#[ignore = "needs QUASAR_AMM_SO; run with --ignored"]
fn locate_the_cu_difference() {
    let n = probes(Framework::Naclac);
    let q = probes(Framework::Quasar);
    println!("\n== where the CU difference sits (Token + Token, every bump 255) ==");

    probe_section(
        "swap a->b",
        ("1 rejected at the first handler line (amount 0)", n.swap_zero_amount, q.swap_zero_amount),
        ("2 rejected after the math (slippage limit)", n.swap_slippage, q.swap_slippage),
        ("3 full run, up to the first token call", n.swap_first_gap, q.swap_first_gap),
    );
    probe_section(
        "add_liquidity",
        ("1 rejected after the share math (zero shares)", n.add_zero_shares, q.add_zero_shares),
        ("2 rejected after all the math (min_lp limit)", n.add_slippage, q.add_slippage),
        ("3 full run, up to the first token call", n.add_first_gap, q.add_first_gap),
    );
    probe_section(
        "remove_liquidity",
        ("1 rejected at the first math line (lp 0)", n.remove_zero_lp, q.remove_zero_lp),
        ("2 rejected after the math (min amount limit)", n.remove_slippage, q.remove_slippage),
        ("3 full run, up to the first token call", n.remove_first_gap, q.remove_first_gap),
    );

    println!("\n   logs of the swap rejected at the first handler line");
    println!("   naclac: {:?}", n.swap_zero_amount_logs);
    println!("   quasar: {:?}", q.swap_zero_amount_logs);
    println!("\n== initialize rejected at the first handler line (amount_a 0): CU by segment ==");
    print_trace_logs("initialize up to the handler", &n.init_zero_amount_logs, &q.init_zero_amount_logs);
}
