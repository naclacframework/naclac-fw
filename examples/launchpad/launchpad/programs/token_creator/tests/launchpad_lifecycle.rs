//! A devnet lifecycle of the launchpad with no assertions: a launch that
//! creates the token and its AMM pool through one CPI, then liquidity added,
//! swaps in both directions and liquidity removed. The cycle runs once with a
//! classic Token quote mint and once with a Token-2022 quote mint, against the
//! programs deployed at the ids in `Naclac.toml`. Every step prints as it
//! happens, with its transaction link, compute units and the payer's balances.

use {
    amm_client::{
        get_pool_state_pda,
        instructions::{
            build_add_liquidity, build_remove_liquidity, build_swap, AddLiquidityAccounts,
            RemoveLiquidityAccounts, SwapAccounts,
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
    std::{
        io::Write,
        time::{Instant, SystemTime, UNIX_EPOCH},
    },
    token_creator_client::{
        instructions::{build_launch_token, LaunchTokenAccounts},
        types::{LaunchTokenArgs, PROGRAM_ID as CREATOR_PROGRAM_ID},
    },
};

const DECIMALS: u8 = 6;
const QUOTE_BALANCE: u64 = 10_000_000_000;
const AMOUNT_TOKEN_POOL: u64 = 1_000_000_000;
const AMOUNT_TOKEN_LAUNCHER: u64 = 500_000_000;
const AMOUNT_QUOTE: u64 = 2_000_000_000;
const ADD_MAX_TOKEN: u64 = 100_000_000;
const ADD_MAX_QUOTE: u64 = 200_000_000;
const SWAP_TOKEN_IN: u64 = 10_000_000;
const SWAP_QUOTE_IN: u64 = 20_000_000;

/// Prints a line to the terminal immediately. It writes to stdout directly, so
/// the test harness does not hold it back until the run ends.
fn say(line: impl std::fmt::Display) {
    let mut out = std::io::stdout().lock();
    writeln!(out, "{line}").expect("Failed to write to stdout");
    out.flush().expect("Failed to flush stdout");
}

/// Runs one transaction, prints how long it took, its compute units and its
/// explorer link, and returns its metadata. Panics with the step's name when
/// the transaction fails.
fn step(
    name: &str,
    send: impl FnOnce() -> Result<NaclacTransactionMetadata, NaclacClientError>,
) -> NaclacTransactionMetadata {
    say(format!("▶ {name}"));
    let started = Instant::now();
    let meta = send().unwrap_or_else(|error| panic!("{name} failed: {error:?}"));
    say(format!(
        "   ✅ {:.1}s · {} CU · https://explorer.solana.com/tx/{}?cluster=devnet",
        started.elapsed().as_secs_f32(),
        meta.compute_units_consumed,
        meta.signature
    ));
    meta
}

fn token_balance(provider: &NaclacProvider, account: &Address) -> u64 {
    AccountFetcher::new(provider)
        .fetch::<TokenAccount>(account)
        .expect("Failed to read a token account")
        .amount()
}

fn find(seeds: &[&[u8]], program: &Address) -> Address {
    Address::find_program_address(seeds, program).0
}

/// A launch id no earlier run used: devnet keeps every launch record, so a
/// repeated id would fail as already initialised.
fn fresh_launch_id() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("The system clock is before the Unix epoch")
        .as_millis() as u64
}

/// Every address a launch of `id` by one payer uses.
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
}

fn derive(payer: &Address, quote_mint: &Address, id: u64) -> Derived {
    let id_bytes = id.to_le_bytes();
    let launch_record = find(
        &[&b"launch"[..], payer.as_ref(), &id_bytes],
        &CREATOR_PROGRAM_ID,
    );
    let mint = find(
        &[&b"mint"[..], payer.as_ref(), &id_bytes],
        &CREATOR_PROGRAM_ID,
    );
    let payer_token_account = find(
        &[payer.as_ref(), TOKEN_PROGRAM_ID.as_ref(), mint.as_ref()],
        &ASSOCIATED_TOKEN_PROGRAM_ID,
    );
    let mint_is_pool_token_a = mint.to_bytes() < quote_mint.to_bytes();
    let (token_a, token_b) = if mint_is_pool_token_a {
        (mint, *quote_mint)
    } else {
        (*quote_mint, mint)
    };
    let (pool_state, _) = get_pool_state_pda(&AMM_PROGRAM_ID, &token_a, &token_b, id);
    let lp_mint = find(&[&b"lp_mint"[..], pool_state.as_ref()], &AMM_PROGRAM_ID);
    let vault_a = find(
        &[&b"vault"[..], pool_state.as_ref(), token_a.as_ref()],
        &AMM_PROGRAM_ID,
    );
    let vault_b = find(
        &[&b"vault"[..], pool_state.as_ref(), token_b.as_ref()],
        &AMM_PROGRAM_ID,
    );
    let payer_lp = find(
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
    }
}

/// One full cycle against a fresh quote mint owned by `quote_program`.
fn run_cycle(label: &str, quote_program: Address) {
    say(format!("\n══ {label} ══"));
    let payer = load_node_wallet().expect("Failed to load local Solana keypair");
    let provider =
        NaclacProvider::new("devnet", payer).expect("Failed to construct NaclacProvider");
    let owner = provider.payer.address();
    let lamports = provider
        .get_balance(&owner)
        .expect("Failed to read the payer's SOL balance");
    say(format!("   👛 payer {owner} · {:.3} SOL", lamports as f64 / 1e9));

    say("▶ create the quote mint and fund the payer");
    let quote_signer = Keypair::new();
    let quote_mint = quote_signer.address();
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
    say(format!("   🪙 quote mint {quote_mint}"));

    let id = fresh_launch_id();
    let derived = derive(&owner, &quote_mint, id);
    let new_token = (derived.mint, derived.payer_token_account, TOKEN_PROGRAM_ID);
    let quote = (quote_mint, payer_quote_account, quote_program);
    let [side_a, side_b] = if derived.mint_is_pool_token_a {
        [new_token, quote]
    } else {
        [quote, new_token]
    };
    let vault_of = |mint: &Address| {
        if (*mint == derived.mint) == derived.mint_is_pool_token_a {
            derived.vault_a
        } else {
            derived.vault_b
        }
    };
    let show_balances = |user_lp: Option<&Address>| {
        say(format!(
            "   💰 new token {} · quote {}{}",
            token_balance(&provider, &derived.payer_token_account),
            token_balance(&provider, &payer_quote_account),
            user_lp
                .map(|lp| format!(" · LP {}", token_balance(&provider, lp)))
                .unwrap_or_default()
        ));
    };

    step("launch the token and create its pool", || {
        build_launch_token(
            &provider,
            CREATOR_PROGRAM_ID,
            LaunchTokenArgs {
                id,
                decimals: DECIMALS,
                amount_token_pool: AMOUNT_TOKEN_POOL,
                amount_token_launcher: AMOUNT_TOKEN_LAUNCHER,
                amount_quote: AMOUNT_QUOTE,
            },
            LaunchTokenAccounts {
                payer: owner,
                quote_mint,
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
                quote_token_program: quote_program,
                associated_token_program: ASSOCIATED_TOKEN_PROGRAM_ID,
                system_program: SYSTEM_PROGRAM_ID,
                amm_program: AMM_PROGRAM_ID,
            },
        )
        .send_and_confirm()
    });
    say(format!(
        "   🏊 pool https://explorer.solana.com/address/{}?cluster=devnet",
        derived.pool_state
    ));
    show_balances(None);

    let lp_signer = Keypair::new();
    let user_lp = lp_signer.address();
    create_token_account(&provider, &lp_signer, &derived.lp_mint, &owner)
        .expect("Failed to create an LP token account");

    let max_for = |side: &(Address, Address, Address)| {
        if side.0 == derived.mint {
            ADD_MAX_TOKEN
        } else {
            ADD_MAX_QUOTE
        }
    };
    step("add liquidity", || {
        build_add_liquidity(
            &provider,
            AMM_PROGRAM_ID,
            max_for(&side_a),
            max_for(&side_b),
            1,
            AddLiquidityAccounts {
                user: owner,
                pool_state: derived.pool_state,
                token_a_mint: side_a.0,
                token_b_mint: side_b.0,
                vault_a: derived.vault_a,
                vault_b: derived.vault_b,
                lp_mint: derived.lp_mint,
                user_token_a: side_a.1,
                user_token_b: side_b.1,
                user_lp,
                token_program_a: side_a.2,
                token_program_b: side_b.2,
                lp_token_program: TOKEN_PROGRAM_ID,
            },
        )
        .send_and_confirm()
    });
    show_balances(Some(&user_lp));

    step("swap the new token for the quote", || {
        build_swap(
            &provider,
            AMM_PROGRAM_ID,
            SWAP_TOKEN_IN,
            1,
            SwapAccounts {
                user: owner,
                pool_state: derived.pool_state,
                token_a_mint: side_a.0,
                token_b_mint: side_b.0,
                pool_source_vault: vault_of(&derived.mint),
                pool_destination_vault: vault_of(&quote_mint),
                user_source_token: derived.payer_token_account,
                user_destination_token: payer_quote_account,
                token_program_a: side_a.2,
                token_program_b: side_b.2,
            },
        )
        .send_and_confirm()
    });
    show_balances(Some(&user_lp));

    step("swap the quote for the new token", || {
        build_swap(
            &provider,
            AMM_PROGRAM_ID,
            SWAP_QUOTE_IN,
            1,
            SwapAccounts {
                user: owner,
                pool_state: derived.pool_state,
                token_a_mint: side_a.0,
                token_b_mint: side_b.0,
                pool_source_vault: vault_of(&quote_mint),
                pool_destination_vault: vault_of(&derived.mint),
                user_source_token: payer_quote_account,
                user_destination_token: derived.payer_token_account,
                token_program_a: side_a.2,
                token_program_b: side_b.2,
            },
        )
        .send_and_confirm()
    });
    show_balances(Some(&user_lp));

    let lp_to_remove = token_balance(&provider, &user_lp) / 2;
    step("remove half of the added liquidity", || {
        build_remove_liquidity(
            &provider,
            AMM_PROGRAM_ID,
            lp_to_remove,
            1,
            1,
            RemoveLiquidityAccounts {
                user: owner,
                pool_state: derived.pool_state,
                token_a_mint: side_a.0,
                token_b_mint: side_b.0,
                vault_a: derived.vault_a,
                vault_b: derived.vault_b,
                lp_mint: derived.lp_mint,
                user_token_a: side_a.1,
                user_token_b: side_b.1,
                user_lp,
                token_program_a: side_a.2,
                token_program_b: side_b.2,
                lp_token_program: TOKEN_PROGRAM_ID,
            },
        )
        .send_and_confirm()
    });
    show_balances(Some(&user_lp));
    say(format!("══ {label} done ══"));
}

#[test]
fn launchpad_lifecycle_on_devnet() {
    run_cycle("classic Token quote mint", TOKEN_PROGRAM_ID);
    run_cycle("Token-2022 quote mint", TOKEN_2022_PROGRAM_ID);
}
