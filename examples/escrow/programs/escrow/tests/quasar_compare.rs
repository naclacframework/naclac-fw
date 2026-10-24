// Runs naclac's `escrow` and Quasar `escrow-state` builds through the same
// litesvm harness, wallet, token setup, seeds and make/cancel/take flows, and
// reports each transaction's compute units split into the program's own and
// those spent in the programs it calls. QUASAR_ESCROW_SO and
// QUASAR_MASTER_ESCROW_SO each name a Quasar escrow_state.so to include; the
// test skips when neither is set. Every program is held to the same end
// state, including the vault being closed.

use {
    escrow_client::{
        get_escrow_state_pda,
        instructions::{
            build_cancel, build_make, build_take, CancelAccounts, MakeAccounts, TakeAccounts,
        },
        types::PROGRAM_ID,
    },
    naclac_client::{
        provider::NaclacTransactionMetadata,
        utils::{
            create_ata_with_program, create_mint_with_program, load_node_wallet,
            mint_to_with_program, transfer_sol, TOKEN_PROGRAM_ID,
        },
        *,
    },
};

const QUASAR_PROGRAM_ID: Address =
    Address::from_str_const("33333333333333333333333333333333333333333333");

#[derive(Clone, Copy)]
enum Kind {
    Naclac,
    Quasar,
}

fn make_ix<'a>(
    kind: Kind,
    provider: &'a NaclacProvider,
    program: Address,
    seed: u64,
    amount_a: u64,
    amount_b: u64,
    accounts: MakeAccounts,
) -> InstructionBuilder<'a> {
    match kind {
        Kind::Naclac => build_make(provider, program, seed, amount_a, amount_b, accounts),
        Kind::Quasar => {
            let mut data = vec![0u8];
            data.extend_from_slice(&seed.to_le_bytes());
            data.extend_from_slice(&amount_a.to_le_bytes());
            data.extend_from_slice(&amount_b.to_le_bytes());
            InstructionBuilder::new(provider, program, data)
                .account(AccountMeta::new(accounts.maker, true), "maker")
                .account(AccountMeta::new(accounts.escrow_state, false), "escrow_state")
                .account(AccountMeta::new_readonly(accounts.mint_a, false), "mint_a")
                .account(AccountMeta::new_readonly(accounts.mint_b, false), "mint_b")
                .account(AccountMeta::new(accounts.vault_token_account, false), "vault")
                .account(AccountMeta::new(accounts.maker_token_account_a, false), "maker_ta_a")
                .account(AccountMeta::new_readonly(accounts.token_program, false), "token")
                .account(AccountMeta::new_readonly(accounts.system_program, false), "system")
        }
    }
}

fn cancel_ix<'a>(
    kind: Kind,
    provider: &'a NaclacProvider,
    program: Address,
    seed: u64,
    accounts: CancelAccounts,
) -> InstructionBuilder<'a> {
    match kind {
        Kind::Naclac => build_cancel(provider, program, seed, accounts),
        Kind::Quasar => {
            let mut data = vec![1u8];
            data.extend_from_slice(&seed.to_le_bytes());
            InstructionBuilder::new(provider, program, data)
                .account(AccountMeta::new(accounts.maker, true), "maker")
                .account(AccountMeta::new_readonly(accounts.mint_a, false), "mint_a")
                .account(AccountMeta::new(accounts.escrow_state, false), "escrow_state")
                .account(AccountMeta::new(accounts.vault_token_account, false), "vault")
                .account(AccountMeta::new(accounts.maker_token_account_a, false), "maker_ta_a")
                .account(AccountMeta::new_readonly(accounts.token_program, false), "token")
                .account(AccountMeta::new_readonly(accounts.system_program, false), "system")
        }
    }
}

fn take_ix<'a>(
    kind: Kind,
    provider: &'a NaclacProvider,
    program: Address,
    seed: u64,
    accounts: TakeAccounts,
) -> InstructionBuilder<'a> {
    match kind {
        Kind::Naclac => build_take(provider, program, seed, accounts),
        Kind::Quasar => {
            let mut data = vec![2u8];
            data.extend_from_slice(&seed.to_le_bytes());
            InstructionBuilder::new(provider, program, data)
                .account(AccountMeta::new(accounts.taker, true), "taker")
                .account(AccountMeta::new(accounts.maker, false), "maker")
                .account(AccountMeta::new_readonly(accounts.mint_a, false), "mint_a")
                .account(AccountMeta::new_readonly(accounts.mint_b, false), "mint_b")
                .account(AccountMeta::new(accounts.escrow_state, false), "escrow_state")
                .account(AccountMeta::new(accounts.vault_token_account, false), "vault")
                .account(AccountMeta::new(accounts.taker_token_account_a, false), "taker_ta_a")
                .account(AccountMeta::new(accounts.taker_token_account_b, false), "taker_ta_b")
                .account(AccountMeta::new(accounts.maker_token_account_b, false), "maker_ta_b")
                .account(AccountMeta::new_readonly(accounts.token_program, false), "token")
                .account(AccountMeta::new_readonly(accounts.system_program, false), "system")
        }
    }
}

/// Splits a transaction's compute units into the program's own and the
/// total reported by the programs it called, read from the `consumed` log
/// lines of every program other than `program`.
fn split_cu(meta: &NaclacTransactionMetadata, program: &Address) -> (u64, u64) {
    let own_id = program.to_string();
    let called: u64 = meta
        .logs
        .iter()
        .filter_map(|line| {
            let rest = line.strip_prefix("Program ")?;
            let (id, rest) = rest.split_once(" consumed ")?;
            if id == own_id {
                return None;
            }
            rest.split_once(" of ")?.0.parse::<u64>().ok()
        })
        .sum();
    (meta.compute_units_consumed.saturating_sub(called), called)
}

fn report(label: &str, step: &str, meta: &NaclacTransactionMetadata, program: &Address) {
    let (own, called) = split_cu(meta, program);
    println!(
        "[{label}] {step}: total {} = program {own} + called programs {called}",
        meta.compute_units_consumed
    );
}

fn token_amount(provider: &NaclacProvider, account: &Address) -> u64 {
    let account = provider.get_account(account).expect("token account exists");
    u64::from_le_bytes(account.data[64..72].try_into().unwrap())
}

/// Two seeds whose escrow PDA bump is 255, the first one tried, for every
/// program compared, so the bump search costs each program the same.
fn seeds_with_first_try_bump(maker: &Address, programs: &[Address]) -> (u64, u64) {
    let mut found = Vec::new();
    let mut seed = 1u64;
    while found.len() < 2 {
        if programs
            .iter()
            .all(|program| get_escrow_state_pda(program, maker, seed).1 == 255)
        {
            found.push(seed);
        }
        seed += 1;
    }
    (found[0], found[1])
}

fn run(label: &str, kind: Kind, program: Address, quasar_so: Option<&str>) {
    let payer = load_node_wallet().expect("Failed to load local Solana keypair");
    let provider = NaclacProvider::new("litesvm", payer).expect("Failed to construct provider");
    if let Some(path) = quasar_so {
        provider
            .add_program(&program, path)
            .expect("Failed to load the Quasar program");
    }
    let token_program = TOKEN_PROGRAM_ID;

    let maker = provider.payer.address();
    let taker = Keypair::new();
    let taker_address = taker.address();
    transfer_sol(&provider, &taker_address, 1_000_000_000).expect("Failed to fund taker");

    let mint_a_signer = Keypair::new();
    let mint_a = mint_a_signer.address();
    create_mint_with_program(&provider, &mint_a_signer, &maker, 9, &token_program)
        .expect("Failed to create mint A");
    let mint_b_signer = Keypair::new();
    let mint_b = mint_b_signer.address();
    create_mint_with_program(&provider, &mint_b_signer, &maker, 9, &token_program)
        .expect("Failed to create mint B");

    let maker_ta_a = create_ata_with_program(&provider, &mint_a, &maker, &token_program).unwrap();
    let maker_ta_b = create_ata_with_program(&provider, &mint_b, &maker, &token_program).unwrap();
    let taker_ta_a =
        create_ata_with_program(&provider, &mint_a, &taker_address, &token_program).unwrap();
    let taker_ta_b =
        create_ata_with_program(&provider, &mint_b, &taker_address, &token_program).unwrap();

    mint_to_with_program(&provider, &mint_a, &maker_ta_a, &provider.payer, 1_000_000_000, &token_program)
        .expect("Failed to mint A");
    mint_to_with_program(&provider, &mint_b, &taker_ta_b, &provider.payer, 2_000_000_000, &token_program)
        .expect("Failed to mint B");

    let (seed_cancel, seed_take) =
        seeds_with_first_try_bump(&maker, &[PROGRAM_ID, QUASAR_PROGRAM_ID]);
    let (escrow_cancel, _)= get_escrow_state_pda(&program, &maker, seed_cancel);
    let vault_cancel =
        create_ata_with_program(&provider, &mint_a, &escrow_cancel, &token_program).unwrap();

    let meta = make_ix(
        kind,
        &provider,
        program,
        seed_cancel,
        400_000_000,
        800_000_000,
        MakeAccounts {
            maker,
            mint_a,
            mint_b,
            escrow_state: escrow_cancel,
            vault_token_account: vault_cancel,
            maker_token_account_a: maker_ta_a,
            token_program,
            system_program: Address::default(),
        },
    )
    .send_and_confirm()
    .unwrap();
    report(label, "make (cancel flow)", &meta, &program);
    assert_eq!(token_amount(&provider, &vault_cancel), 400_000_000);

    let meta = cancel_ix(
        kind,
        &provider,
        program,
        seed_cancel,
        CancelAccounts {
            maker,
            mint_a,
            escrow_state: escrow_cancel,
            vault_token_account: vault_cancel,
            maker_token_account_a: maker_ta_a,
            token_program,
            system_program: Address::default(),
        },
    )
    .send_and_confirm()
    .unwrap();
    report(label, "cancel", &meta, &program);
    assert_eq!(token_amount(&provider, &maker_ta_a), 1_000_000_000);
    assert!(provider.get_account(&escrow_cancel).is_err(), "escrow not closed");
    assert!(provider.get_account(&vault_cancel).is_err(), "vault not closed");

    let (escrow_take, _) = get_escrow_state_pda(&program, &maker, seed_take);
    let vault_take =
        create_ata_with_program(&provider, &mint_a, &escrow_take, &token_program).unwrap();

    let meta = make_ix(
        kind,
        &provider,
        program,
        seed_take,
        500_000_000,
        1_000_000_000,
        MakeAccounts {
            maker,
            mint_a,
            mint_b,
            escrow_state: escrow_take,
            vault_token_account: vault_take,
            maker_token_account_a: maker_ta_a,
            token_program,
            system_program: Address::default(),
        },
    )
    .send_and_confirm()
    .unwrap();
    report(label, "make (take flow)", &meta, &program);
    assert_eq!(token_amount(&provider, &vault_take), 500_000_000);

    let meta = take_ix(
        kind,
        &provider,
        program,
        seed_take,
        TakeAccounts {
            taker: taker_address,
            maker,
            mint_a,
            mint_b,
            escrow_state: escrow_take,
            vault_token_account: vault_take,
            taker_token_account_a: taker_ta_a,
            taker_token_account_b: taker_ta_b,
            maker_token_account_b: maker_ta_b,
            token_program,
            system_program: Address::default(),
        },
    )
    .signer(&taker)
    .send_and_confirm()
    .unwrap();
    report(label, "take", &meta, &program);
    assert_eq!(token_amount(&provider, &taker_ta_a), 500_000_000);
    assert_eq!(token_amount(&provider, &maker_ta_b), 1_000_000_000);
    assert!(provider.get_account(&escrow_take).is_err(), "escrow not closed");
    assert!(provider.get_account(&vault_take).is_err(), "vault not closed");
}

#[test]
fn compare_with_quasar_escrow() {
    let release = std::env::var("QUASAR_ESCROW_SO").ok();
    let master = std::env::var("QUASAR_MASTER_ESCROW_SO").ok();
    if release.is_none() && master.is_none() {
        println!("QUASAR_ESCROW_SO and QUASAR_MASTER_ESCROW_SO are not set; skipping");
        return;
    }
    run("naclac", Kind::Naclac, PROGRAM_ID, None);
    if let Some(so) = release {
        run("quasar-release", Kind::Quasar, QUASAR_PROGRAM_ID, Some(&so));
    }
    if let Some(so) = master {
        run("quasar-master", Kind::Quasar, QUASAR_PROGRAM_ID, Some(&so));
    }
}
