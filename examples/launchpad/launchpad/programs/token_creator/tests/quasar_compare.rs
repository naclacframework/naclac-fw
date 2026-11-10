//! Runs the same token launch against the naclac token creator on the naclac
//! AMM and the Quasar token creator (`quasar-master/examples/token_creator`)
//! on the Quasar AMM (`quasar-master/examples/amm`) in one litesvm harness,
//! checks that both leave the same balances and accounts, and prints compute
//! units per launch and the size of each `.so`.
//!
//! The Quasar binaries are read from `QUASAR_TOKEN_CREATOR_SO` and
//! `QUASAR_AMM_SO`; the naclac binaries are the ones `naclac build` leaves in
//! `artifacts/deploy`, so run `naclac build -p amm` and
//! `naclac build -p token_creator` first.

use {
    amm_client::{get_pool_state_pda, types::PROGRAM_ID as NACLAC_AMM_ID},
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
        types::{LaunchTokenArgs, PROGRAM_ID as NACLAC_CREATOR_ID},
    },
};

const QUASAR_AMM_ID: Address = address!("55555555555555555555555555555555555555555555");
const QUASAR_CREATOR_ID: Address = address!("66666666666666666666666666666666666666666666");

const DECIMALS: u8 = 6;
const QUOTE_BALANCE: u64 = 10_000_000_000;
const AMOUNT_TOKEN_POOL: u64 = 1_000_000_000;
const AMOUNT_TOKEN_LAUNCHER: u64 = 500_000_000;
const AMOUNT_QUOTE: u64 = 2_000_000_000;

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

    fn creator_id(self) -> Address {
        match self {
            Framework::Naclac => NACLAC_CREATOR_ID,
            Framework::Quasar => QUASAR_CREATOR_ID,
        }
    }

    fn amm_id(self) -> Address {
        match self {
            Framework::Naclac => NACLAC_AMM_ID,
            Framework::Quasar => QUASAR_AMM_ID,
        }
    }
}

fn find(seeds: &[&[u8]], program: &Address) -> (Address, u8) {
    Address::find_program_address(seeds, program)
}

/// Every address a launch of `id` by one payer uses, with the canonical bump
/// of each address that is searched for on chain.
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

fn derive(framework: Framework, payer: &Address, quote_mint: &Address, id: u64) -> Derived {
    let creator = framework.creator_id();
    let amm = framework.amm_id();
    let id_bytes = id.to_le_bytes();
    let (launch_record, launch_record_bump) =
        find(&[&b"launch"[..], payer.as_ref(), &id_bytes], &creator);
    let (mint, mint_bump) = find(&[&b"mint"[..], payer.as_ref(), &id_bytes], &creator);
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
    let (pool_state, pool_bump) = match framework {
        Framework::Naclac => get_pool_state_pda(&amm, &token_a, &token_b, id),
        Framework::Quasar => find(
            &[&b"pool"[..], token_a.as_ref(), token_b.as_ref(), &id_bytes],
            &amm,
        ),
    };
    let (lp_mint, lp_mint_bump) = find(&[&b"lp_mint"[..], pool_state.as_ref()], &amm);
    let (vault_a, vault_a_bump) =
        find(&[&b"vault"[..], pool_state.as_ref(), token_a.as_ref()], &amm);
    let (vault_b, vault_b_bump) =
        find(&[&b"vault"[..], pool_state.as_ref(), token_b.as_ref()], &amm);
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

#[derive(Debug, PartialEq)]
struct Outcome {
    payer_token: u64,
    payer_quote: u64,
    vault_a: u64,
    vault_b: u64,
    payer_lp: u64,
    payer_lp_locked_to_launch_record: bool,
    mint_supply: u64,
    mint_authority_revoked: bool,
    lp_supply: u64,
    record_creator_is_payer: bool,
    record_mint_is_mint: bool,
    record_amount_token: u64,
    record_amount_quote: u64,
}

struct Env {
    framework: Framework,
    provider: NaclacProvider,
    id: u64,
    derived: Derived,
    quote_mint: Address,
    quote_program: Address,
    payer_quote_account: Address,
}

fn quasar_path(variable: &str) -> String {
    std::env::var(variable).unwrap_or_else(|_| {
        panic!("{variable} is not set; point it at the built Quasar binary under quasar-master/target/deploy")
    })
}

fn naclac_so_path(name: &str) -> String {
    format!(
        "{}/../../artifacts/deploy/{name}.so",
        env!("CARGO_MANIFEST_DIR")
    )
}

fn file_size(path: &str) -> u64 {
    std::fs::metadata(path)
        .unwrap_or_else(|e| panic!("cannot read {path}: {e}"))
        .len()
}

impl Env {
    /// A funded quote mint under `quote_program`, and the smallest launch `id`
    /// for which every address search ends at bump 255 and the quote mint
    /// sorts below the new mint exactly when `quote_below_mint` is set.
    fn new(framework: Framework, quote_program: Address, quote_below_mint: bool) -> Env {
        let payer = load_node_wallet().expect("Failed to load local Solana keypair");
        let provider =
            NaclacProvider::new("litesvm", payer).expect("Failed to construct NaclacProvider");
        if framework == Framework::Quasar {
            provider
                .add_program(&QUASAR_AMM_ID, &quasar_path("QUASAR_AMM_SO"))
                .expect("Failed to load the Quasar AMM binary");
            provider
                .add_program(&QUASAR_CREATOR_ID, &quasar_path("QUASAR_TOKEN_CREATOR_SO"))
                .expect("Failed to load the Quasar token creator binary");
        }
        let owner = provider.payer.address();

        let quote_signer = Keypair::new();
        let quote_mint = quote_signer.address();
        let (id, derived) = (0u64..)
            .map(|id| (id, derive(framework, &owner, &quote_mint, id)))
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
        Env { framework, provider, id, derived, quote_mint, quote_program, payer_quote_account }
    }

    fn payer(&self) -> Address {
        self.provider.payer.address()
    }

    fn launch(&self) -> Result<NaclacTransactionMetadata, NaclacClientError> {
        let d = &self.derived;
        match self.framework {
            Framework::Naclac => build_launch_token(
                &self.provider,
                NACLAC_CREATOR_ID,
                LaunchTokenArgs {
                    id: self.id,
                    decimals: DECIMALS,
                    amount_token_pool: AMOUNT_TOKEN_POOL,
                    amount_token_launcher: AMOUNT_TOKEN_LAUNCHER,
                    amount_quote: AMOUNT_QUOTE,
                },
                LaunchTokenAccounts {
                    payer: self.payer(),
                    quote_mint: self.quote_mint,
                    launch_record: d.launch_record,
                    mint: d.mint,
                    payer_token_account: d.payer_token_account,
                    payer_quote_account: self.payer_quote_account,
                    pool_state: d.pool_state,
                    pool_vault_a: d.vault_a,
                    pool_vault_b: d.vault_b,
                    pool_lp_mint: d.lp_mint,
                    payer_lp: d.payer_lp,
                    token_program: TOKEN_PROGRAM_ID,
                    quote_token_program: self.quote_program,
                    associated_token_program: ASSOCIATED_TOKEN_PROGRAM_ID,
                    system_program: SYSTEM_PROGRAM_ID,
                    amm_program: NACLAC_AMM_ID,
                },
            )
            .send_and_confirm(),
            Framework::Quasar => {
                let mut data = vec![1, 0, 0, 0, 0, 0, 0, 0];
                data.extend_from_slice(&self.id.to_le_bytes());
                data.push(DECIMALS);
                data.extend_from_slice(&AMOUNT_TOKEN_POOL.to_le_bytes());
                data.extend_from_slice(&AMOUNT_TOKEN_LAUNCHER.to_le_bytes());
                data.extend_from_slice(&AMOUNT_QUOTE.to_le_bytes());
                InstructionBuilder::new(&self.provider, QUASAR_CREATOR_ID, data)
                    .accounts(vec![
                        (AccountMeta::new(self.payer(), true), "payer"),
                        (AccountMeta::new_readonly(self.quote_mint, false), "quote_mint"),
                        (AccountMeta::new(d.launch_record, false), "launch_record"),
                        (AccountMeta::new(d.mint, false), "mint"),
                        (AccountMeta::new(d.payer_token_account, false), "payer_token_account"),
                        (AccountMeta::new(self.payer_quote_account, false), "payer_quote_account"),
                        (AccountMeta::new(d.pool_state, false), "pool_state"),
                        (AccountMeta::new(d.vault_a, false), "pool_vault_a"),
                        (AccountMeta::new(d.vault_b, false), "pool_vault_b"),
                        (AccountMeta::new(d.lp_mint, false), "pool_lp_mint"),
                        (AccountMeta::new(d.payer_lp, false), "payer_lp"),
                        (AccountMeta::new_readonly(TOKEN_PROGRAM_ID, false), "token_program"),
                        (
                            AccountMeta::new_readonly(self.quote_program, false),
                            "quote_token_program",
                        ),
                        (
                            AccountMeta::new_readonly(ASSOCIATED_TOKEN_PROGRAM_ID, false),
                            "associated_token_program",
                        ),
                        (AccountMeta::new_readonly(SYSTEM_PROGRAM_ID, false), "system_program"),
                        (AccountMeta::new_readonly(QUASAR_AMM_ID, false), "amm_program"),
                    ])
                    .send_and_confirm()
            }
        }
    }

    fn token_account(&self, address: &Address) -> TokenAccount {
        AccountFetcher::new(&self.provider)
            .fetch::<TokenAccount>(address)
            .expect("Failed to read token account")
    }

    fn mint_account(&self, address: &Address) -> Mint {
        AccountFetcher::new(&self.provider)
            .fetch::<Mint>(address)
            .expect("Failed to read mint")
    }

    fn data(&self, address: &Address) -> Vec<u8> {
        self.provider
            .get_account_data(address)
            .expect("Failed to read account data")
    }

    fn outcome(&self) -> Outcome {
        let d = &self.derived;
        let mint_data = self.data(&d.mint);
        let record = self.data(&d.launch_record);
        let record = &record[8..];
        Outcome {
            payer_token: self.token_account(&d.payer_token_account).amount(),
            payer_quote: self.token_account(&self.payer_quote_account).amount(),
            vault_a: self.token_account(&d.vault_a).amount(),
            vault_b: self.token_account(&d.vault_b).amount(),
            payer_lp: self.token_account(&d.payer_lp).amount(),
            payer_lp_locked_to_launch_record: self.token_account(&d.payer_lp).owner()
                == d.launch_record,
            mint_supply: self.mint_account(&d.mint).supply(),
            mint_authority_revoked: mint_data[0..4] == [0, 0, 0, 0],
            lp_supply: self.mint_account(&d.lp_mint).supply(),
            record_creator_is_payer: record[0..32] == self.payer().to_bytes(),
            record_mint_is_mint: record[32..64] == d.mint.to_bytes(),
            record_amount_token: u64::from_le_bytes(record[64..72].try_into().unwrap()),
            record_amount_quote: u64::from_le_bytes(record[72..80].try_into().unwrap()),
        }
    }
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
    let n = trace(&NACLAC_CREATOR_ID, &naclac.logs);
    let q = trace(&QUASAR_CREATOR_ID, &quasar.logs);
    println!("   -- {label}: program-only CU {} vs {} --", n.total, q.total);
    let same_calls = n.children.len() == q.children.len()
        && n.children.iter().zip(&q.children).all(|(a, b)| {
            let (a_id, b_id) = (a.0.as_str(), b.0.as_str());
            let is_amm = |id: &str, amm: &Address| amm.to_string().starts_with(&id[..8]);
            a_id == b_id
                || (is_amm(a_id, &NACLAC_AMM_ID) && is_amm(b_id, &QUASAR_AMM_ID))
        });
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

fn print_row(label: &str, naclac: u64, quasar: u64) {
    println!(
        "   {label:<30} {naclac:>8} {quasar:>8} {:>+8}",
        i128::from(quasar) - i128::from(naclac)
    );
}

/// The creator's own compute units, the AMM call's (with everything the AMM
/// calls), and the remaining calls to the system, Token and associated-token
/// programs, which cost the same in both.
fn split(framework: Framework, meta: &NaclacTransactionMetadata) -> (u64, u64, u64) {
    let t = trace(&framework.creator_id(), &meta.logs);
    let amm_prefix: String = framework.amm_id().to_string().chars().take(8).collect();
    let amm: u64 = t
        .children
        .iter()
        .filter(|(id, _)| id.starts_with(&amm_prefix))
        .map(|(_, cu)| cu)
        .sum();
    let other: u64 = t
        .children
        .iter()
        .filter(|(id, _)| !id.starts_with(&amm_prefix))
        .map(|(_, cu)| cu)
        .sum();
    (t.total - amm - other, amm, other)
}

fn compare(label: &str, quote_program: &Address, quote_below_mint: bool, detail: bool) {
    let run = |framework: Framework| {
        let env = Env::new(framework, *quote_program, quote_below_mint);
        let meta = env.launch().expect("launch failed");
        println!("   {} launch id {}", framework.name(), env.id);
        (meta, env.outcome())
    };
    println!("\n== {label} ==");
    let (naclac_meta, naclac_outcome) = run(Framework::Naclac);
    let (quasar_meta, quasar_outcome) = run(Framework::Quasar);

    assert_eq!(
        naclac_outcome, quasar_outcome,
        "both programs must leave the same balances and accounts"
    );
    assert!(naclac_outcome.mint_authority_revoked && naclac_outcome.payer_lp_locked_to_launch_record);

    let (n_own, n_amm, n_other) = split(Framework::Naclac, &naclac_meta);
    let (q_own, q_amm, q_other) = split(Framework::Quasar, &quasar_meta);
    println!("   {:<30} {:>8} {:>8} {:>8}", "launch_token", "naclac", "quasar", "delta");
    print_row("whole transaction", naclac_meta.compute_units_consumed, quasar_meta.compute_units_consumed);
    print_row("creator's own code", n_own, q_own);
    print_row("AMM initialize (whole call)", n_amm, q_amm);
    print_row("system/token/ata calls", n_other, q_other);

    if detail {
        print_trace("launch_token", &naclac_meta, &quasar_meta);
    }
}

#[test]
#[ignore = "needs QUASAR_AMM_SO and QUASAR_TOKEN_CREATOR_SO; run with --ignored"]
fn compare_naclac_and_quasar_token_creator() {
    println!("\n== program size (bytes) ==");
    println!(
        "   token creator: naclac {}   quasar {}",
        file_size(&naclac_so_path("token_creator")),
        file_size(&quasar_path("QUASAR_TOKEN_CREATOR_SO"))
    );
    println!(
        "   amm:           naclac {}   quasar {}",
        file_size(&naclac_so_path("amm")),
        file_size(&quasar_path("QUASAR_AMM_SO"))
    );

    compare("Token quote, quote below the new mint", &TOKEN_PROGRAM_ID, true, true);
    compare("Token quote, quote above the new mint", &TOKEN_PROGRAM_ID, false, false);
    compare("Token-2022 quote, quote below the new mint", &TOKEN_2022_PROGRAM_ID, true, false);
    compare("Token-2022 quote, quote above the new mint", &TOKEN_2022_PROGRAM_ID, false, false);
}
