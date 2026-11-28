//! Runs the same treasury scenario against the naclac program and its port in
//! Quasar or Anchor v2 in one litesvm harness, driving both with the
//! naclac-generated builders, checks that both end in the same state, and
//! prints compute units per step and the size of each `.so`.
//!
//! The Quasar binary is read from `QUASAR_TREASURY_SO` and the Anchor binary
//! from `ANCHOR_TREASURY_SO`; the naclac binary is the one `naclac build`
//! leaves in `artifacts/deploy`. The admin and the note ids are chosen so
//! every PDA of the scenario has bump 255 under all three program ids, so no
//! side pays for a longer canonical bump search. Anchor accounts carry an
//! 8-byte discriminator, so their state is decoded at that offset and their
//! lengths are compared without the 7 extra bytes.

use {
    naclac_client::*,
    std::cell::RefCell,
    treasury_client::{
        get_event_authority_pda, get_note_pda, get_proposal_pda, get_treasury_pda, get_vault_pda,
        instructions::{
            build_add_member, build_approve, build_close_proposal, build_create_treasury,
            build_deposit, build_execute, build_init_note, build_merge_notes, build_propose,
            build_remove_member, build_resize_note, build_set_status, AddMemberAccounts,
            ApproveAccounts, CloseProposalAccounts, CreateTreasuryAccounts, DepositAccounts,
            ExecuteAccounts, InitNoteAccounts, MergeNotesAccounts, ProposeAccounts,
            RemoveMemberAccounts, ResizeNoteAccounts, SetStatusAccounts,
        },
        state::{fetch_note, fetch_proposal, fetch_treasury},
        types::{Status, PROGRAM_ID},
    },
};

const QUASAR_ID: Address =
    Address::from_str_const("77777777777777777777777777777777777777777777");
const ANCHOR_ID: Address =
    Address::from_str_const("88888888888888888888888888888888888888888888");

const NOW: i64 = 1_000_000;
const LATER: i64 = NOW + 3_600;
const DEPOSIT: u64 = 5_000_000;
const PAYOUT: u64 = 1_000_000;
const NOTE_SIZE: u64 = 100;

#[derive(Clone, Copy, PartialEq)]
enum Framework {
    Naclac,
    Quasar,
    Anchor,
}

impl Framework {
    fn label(self) -> &'static str {
        match self {
            Framework::Naclac => "naclac",
            Framework::Quasar => "quasar",
            Framework::Anchor => "anchor",
        }
    }

    fn program(self) -> Address {
        match self {
            Framework::Naclac => PROGRAM_ID,
            Framework::Quasar => QUASAR_ID,
            Framework::Anchor => ANCHOR_ID,
        }
    }

    /// The bytes an account's discriminator takes beyond naclac's one byte.
    fn discriminator_overhead(self) -> usize {
        match self {
            Framework::Anchor => 7,
            Framework::Naclac | Framework::Quasar => 0,
        }
    }
}

struct Keys {
    admin: Keypair,
    member_a: Keypair,
    member_b: Keypair,
    member_c: Address,
    recipient: Address,
    note_one: u64,
    note_two: u64,
}

fn all_first_try(admin: &Address) -> bool {
    [PROGRAM_ID, QUASAR_ID, ANCHOR_ID].iter().all(|program| {
        let (treasury, treasury_bump) = get_treasury_pda(program, admin);
        let (_, vault_bump) = get_vault_pda(program, &treasury);
        let (_, proposal_bump) = get_proposal_pda(program, &treasury, 0);
        treasury_bump == 255 && vault_bump == 255 && proposal_bump == 255
    })
}

fn note_id_with_first_try_bump(authority: &Address, first: u64) -> u64 {
    (first..)
        .find(|id| {
            [PROGRAM_ID, QUASAR_ID, ANCHOR_ID]
                .iter()
                .all(|program| get_note_pda(program, authority, *id).1 == 255)
        })
        .expect("a note id with bump 255 exists")
}

impl Keys {
    fn new() -> Keys {
        let admin = loop {
            let candidate = Keypair::new();
            if all_first_try(&candidate.address()) {
                break candidate;
            }
        };
        let member_a = Keypair::new();
        let note_one = note_id_with_first_try_bump(&member_a.address(), 1);
        let note_two = note_id_with_first_try_bump(&member_a.address(), note_one + 1);
        Keys {
            admin,
            member_a,
            member_b: Keypair::new(),
            member_c: Keypair::new().address(),
            recipient: Keypair::new().address(),
            note_one,
            note_two,
        }
    }
}

struct Row {
    step: &'static str,
    cu: u64,
}

struct Outcome {
    framework: Framework,
    rows: Vec<Row>,
    facts: Vec<String>,
}

fn file_size(path: &str) -> u64 {
    std::fs::metadata(path)
        .unwrap_or_else(|e| panic!("cannot read {path}: {e}"))
        .len()
}

fn other_so_path(framework: Framework) -> String {
    let (variable, example) = match framework {
        Framework::Quasar => (
            "QUASAR_TREASURY_SO",
            "/path/to/quasar-master/target/deploy/quasar_treasury.so",
        ),
        Framework::Anchor => (
            "ANCHOR_TREASURY_SO",
            "/path/to/anchor/target/deploy/treasury_v2.so",
        ),
        Framework::Naclac => panic!("the naclac binary comes from `naclac build`"),
    };
    std::env::var(variable).unwrap_or_else(|_| {
        panic!(
            "{variable} is not set; point it at the built treasury binary, for example \
             {variable}={example}"
        )
    })
}

fn naclac_so_path() -> String {
    format!(
        "{}/../../artifacts/deploy/treasury.so",
        env!("CARGO_MANIFEST_DIR")
    )
}

struct Run<'a> {
    framework: Framework,
    program: Address,
    provider: NaclacProvider,
    keys: &'a Keys,
    treasury: Address,
    vault: Address,
    vault_bump: u8,
    rows: RefCell<Vec<Row>>,
}

impl<'a> Run<'a> {
    fn new(framework: Framework, keys: &'a Keys, other_so: &str) -> Run<'a> {
        let payer = load_node_wallet().expect("Failed to load local Solana keypair");
        let provider =
            NaclacProvider::new("litesvm", payer).expect("Failed to construct NaclacProvider");
        let program = framework.program();
        if framework != Framework::Naclac {
            provider.add_program(&program, other_so).unwrap_or_else(|e| {
                panic!("Failed to load the {} treasury binary: {e:?}", framework.label())
            });
        }
        provider
            .set_clock_unix_timestamp(NOW)
            .expect("Failed to set the clock");
        for funded in [
            keys.admin.address(),
            keys.member_a.address(),
            keys.member_b.address(),
        ] {
            transfer_sol(&provider, &funded, 5_000_000_000).expect("Failed to fund an account");
        }
        let (treasury, _) = get_treasury_pda(&program, &keys.admin.address());
        let (vault, vault_bump) = get_vault_pda(&program, &treasury);
        Run {
            framework,
            program,
            provider,
            keys,
            treasury,
            vault,
            vault_bump,
            rows: RefCell::new(Vec::new()),
        }
    }

    fn send(&self, step: &'static str, builder: InstructionBuilder<'_>) -> NaclacTransactionMetadata {
        self.provider
            .expire_blockhash()
            .expect("Failed to expire the blockhash");
        let meta = builder
            .send_and_confirm()
            .unwrap_or_else(|e| panic!("[{}] {step} failed: {e:?}", self.framework.label()));
        self.rows.borrow_mut().push(Row {
            step,
            cu: meta.compute_units_consumed,
        });
        meta
    }

    /// Sends a transaction that must fail and returns the compute units the
    /// program reported consuming before it failed, with the instruction error.
    fn send_failing(&self, builder: InstructionBuilder<'_>) -> (u64, String) {
        self.provider
            .expire_blockhash()
            .expect("Failed to expire the blockhash");
        match builder.send_and_confirm() {
            Err(NaclacClientError::TransactionFailed {
                instruction_err,
                logs,
                ..
            }) => {
                let prefix = format!("Program {} consumed ", self.program);
                let consumed = logs
                    .iter()
                    .rev()
                    .find_map(|line| line.strip_prefix(prefix.as_str()))
                    .and_then(|rest| rest.split_whitespace().next())
                    .and_then(|number| number.parse::<u64>().ok())
                    .unwrap_or_else(|| {
                        panic!("[{}] no consumed line in {logs:?}", self.framework.label())
                    });
                (consumed, format!("{instruction_err:?}"))
            }
            other => panic!(
                "[{}] the transaction should have failed, got {other:?}",
                self.framework.label()
            ),
        }
    }

    fn proposal(&self) -> Address {
        get_proposal_pda(&self.program, &self.treasury, 0).0
    }

    fn note(&self, id: u64) -> Address {
        get_note_pda(&self.program, &self.keys.member_a.address(), id).0
    }

    fn lamports(&self, address: &Address) -> u64 {
        self.provider
            .get_account(address)
            .expect("the account should exist")
            .lamports
    }

    fn account_bytes(&self, address: &Address) -> Vec<u8> {
        self.provider
            .get_account(address)
            .expect("the account should exist")
            .data
    }

    fn raw_len(&self, address: &Address) -> usize {
        self.account_bytes(address).len()
    }

    fn data_len(&self, address: &Address) -> usize {
        self.raw_len(address) - self.framework.discriminator_overhead()
    }

    fn proposal_fact(&self) -> String {
        let address = self.proposal();
        if self.framework == Framework::Anchor {
            let data = self.account_bytes(&address);
            let approvals = addresses_at(&data, 132, usize::from(u16_at(&data, 130)));
            return format!(
                "proposal id={} amount={} deadline={} executed={} approvals={:?} \
                 proposer={} recipient={} len={}",
                u64_at(&data, 40),
                u64_at(&data, 112),
                i64::from_le_bytes(array_at(&data, 120)),
                data[128] != 0,
                approvals,
                address_at(&data, 48),
                address_at(&data, 80),
                self.data_len(&address),
            );
        }
        let proposal = fetch_proposal(&self.provider, &address).expect("proposal should be readable");
        let (id, amount, deadline) = (proposal.id, proposal.amount, proposal.deadline);
        format!(
            "proposal id={id} amount={amount} deadline={deadline} executed={} approvals={:?} \
             proposer={} recipient={} len={}",
            bool::from(proposal.executed),
            proposal.approvals.iter().collect::<Vec<_>>(),
            proposal.proposer,
            proposal.recipient,
            self.data_len(&address),
        )
    }

    fn treasury_fact(&self) -> String {
        if self.framework == Framework::Anchor {
            let data = self.account_bytes(&self.treasury);
            let status = match data[43] {
                0 => Status::Active,
                1 => Status::Paused,
                other => panic!("the treasury status {other} is not valid"),
            };
            let members = addresses_at(&data, 62, usize::from(u16_at(&data, 60)));
            return format!(
                "treasury admin={} threshold={} status={status:?} next_id={} \
                 total_deposited={} members={:?} len={}",
                address_at(&data, 8),
                data[42],
                u64_at(&data, 44),
                u64_at(&data, 52),
                members,
                self.data_len(&self.treasury),
            );
        }
        let treasury =
            fetch_treasury(&self.provider, &self.treasury).expect("treasury should be readable");
        let (threshold, next_id, total_deposited) =
            (treasury.threshold, treasury.next_id, treasury.total_deposited);
        let Ok(status) = treasury.status.get() else {
            panic!("the treasury status should be valid");
        };
        format!(
            "treasury admin={} threshold={threshold} status={status:?} next_id={next_id} \
             total_deposited={total_deposited} members={:?} len={}",
            treasury.admin,
            treasury.members.iter().collect::<Vec<_>>(),
            self.data_len(&self.treasury),
        )
    }

    /// A note's length is the program's own size unless `resized` says an
    /// instruction set it, in which case every framework reports the same raw
    /// length.
    fn note_fact(&self, name: &str, id: u64, resized: bool) -> String {
        let address = self.note(id);
        let len = if resized {
            self.raw_len(&address)
        } else {
            self.data_len(&address)
        };
        if self.framework == Framework::Anchor {
            let data = self.account_bytes(&address);
            return format!(
                "{name} authority={} id={} count={} len={len}",
                address_at(&data, 8),
                u64_at(&data, 40),
                u64_at(&data, 48),
            );
        }
        let note = fetch_note(&self.provider, &address).expect("note should be readable");
        let (note_id, count) = (note.id, note.count);
        format!(
            "{name} authority={} id={note_id} count={count} len={len}",
            note.authority,
        )
    }
}

fn array_at<const N: usize>(data: &[u8], offset: usize) -> [u8; N] {
    data[offset..offset + N]
        .try_into()
        .expect("the account is long enough")
}

fn u16_at(data: &[u8], offset: usize) -> u16 {
    u16::from_le_bytes(array_at(data, offset))
}

fn u64_at(data: &[u8], offset: usize) -> u64 {
    u64::from_le_bytes(array_at(data, offset))
}

fn address_at(data: &[u8], offset: usize) -> Address {
    Address::new_from_array(array_at(data, offset))
}

fn addresses_at(data: &[u8], offset: usize, count: usize) -> Vec<Address> {
    (0..count)
        .map(|index| address_at(data, offset + 32 * index))
        .collect()
}

fn run(framework: Framework, keys: &Keys, other_so: &str) -> Outcome {
    let run = Run::new(framework, keys, other_so);
    let program = run.program;
    let provider = &run.provider;
    let admin = &keys.admin;
    let member_a = &keys.member_a;
    let member_b = &keys.member_b;
    let mut facts = Vec::new();

    run.send(
        "create_treasury",
        build_create_treasury(
            provider,
            program,
            run.vault_bump,
            2,
            CreateTreasuryAccounts {
                admin: admin.address(),
                treasury: run.treasury,
                vault: run.vault,
                system_program: Address::default(),
            },
        )
        .signer(admin),
    );

    for (step, member) in [
        ("add_member", member_a.address()),
        ("add_member", member_b.address()),
    ] {
        run.send(
            step,
            build_add_member(
                provider,
                program,
                member,
                AddMemberAccounts {
                    admin: admin.address(),
                    treasury: run.treasury,
                },
            )
            .signer(admin),
        );
    }

    for status in [Status::Paused, Status::Active] {
        run.send(
            "set_status",
            build_set_status(
                provider,
                program,
                status,
                SetStatusAccounts {
                    admin: admin.address(),
                    treasury: run.treasury,
                },
            )
            .signer(admin),
        );
    }

    run.send(
        "deposit",
        build_deposit(
            provider,
            program,
            DEPOSIT,
            DepositAccounts {
                depositor: admin.address(),
                treasury: run.treasury,
                vault: run.vault,
                system_program: Address::default(),
            },
        )
        .signer(admin),
    );

    for id in [keys.note_one, keys.note_two] {
        run.send(
            "init_note",
            build_init_note(
                provider,
                program,
                id,
                InitNoteAccounts {
                    authority: member_a.address(),
                    note: run.note(id),
                    system_program: Address::default(),
                },
            )
            .signer(member_a),
        );
    }

    run.send(
        "propose",
        build_propose(
            provider,
            program,
            0,
            keys.recipient,
            PAYOUT,
            LATER,
            ProposeAccounts {
                proposer: member_a.address(),
                treasury: run.treasury,
                proposal: run.proposal(),
                note: Some(run.note(keys.note_one)),
                system_program: Address::default(),
            },
        )
        .signer(member_a),
    );

    for signer in [member_a, member_b] {
        run.send(
            "approve",
            build_approve(
                provider,
                program,
                ApproveAccounts {
                    treasury: run.treasury,
                    proposal: run.proposal(),
                },
            )
            .remaining_accounts(vec![AccountMeta::new_readonly(signer.address(), true)])
            .signer(signer),
        );
    }

    let executed = run.send(
        "execute",
        build_execute(
            provider,
            program,
            ExecuteAccounts {
                executor: provider.payer.address(),
                treasury: run.treasury,
                proposal: run.proposal(),
                vault: run.vault,
                recipient: keys.recipient,
                system_program: Address::default(),
                event_authority: get_event_authority_pda(&program).0,
                program,
            },
        ),
    );
    let returned = u64::from_le_bytes(
        executed.return_data[..8]
            .try_into()
            .expect("the return data is a u64"),
    );
    facts.push(format!("execute returned {returned}"));

    facts.push(run.proposal_fact());

    run.send(
        "close_proposal",
        build_close_proposal(
            provider,
            program,
            CloseProposalAccounts {
                proposer: member_a.address(),
                proposal: run.proposal(),
            },
        )
        .signer(member_a),
    );

    run.send(
        "merge_notes",
        build_merge_notes(
            provider,
            program,
            1,
            MergeNotesAccounts {
                authority: member_a.address(),
                from: run.note(keys.note_one),
                to: run.note(keys.note_two),
            },
        )
        .signer(member_a),
    );

    run.send(
        "resize_note",
        build_resize_note(
            provider,
            program,
            NOTE_SIZE,
            ResizeNoteAccounts {
                authority: member_a.address(),
                note: run.note(keys.note_two),
                system_program: Address::default(),
            },
        )
        .signer(member_a),
    );

    run.send(
        "add_member",
        build_add_member(
            provider,
            program,
            keys.member_c,
            AddMemberAccounts {
                admin: admin.address(),
                treasury: run.treasury,
            },
        )
        .signer(admin),
    );

    run.send(
        "remove_member",
        build_remove_member(
            provider,
            program,
            member_b.address(),
            RemoveMemberAccounts {
                admin: admin.address(),
                treasury: run.treasury,
            },
        )
        .signer(admin),
    );

    facts.push(run.treasury_fact());

    for (name, note_id, resized) in [
        ("note_one", keys.note_one, false),
        ("note_two", keys.note_two, true),
    ] {
        facts.push(run.note_fact(name, note_id, resized));
    }

    facts.push(format!(
        "vault lamports={} recipient lamports={}",
        run.lamports(&run.vault),
        run.lamports(&keys.recipient),
    ));

    let rows = run.rows.into_inner();
    Outcome {
        framework,
        rows,
        facts,
    }
}

fn verdict(naclac: u64, other: u64, other_label: &str) -> String {
    if naclac < other {
        format!("naclac -{}", other - naclac)
    } else if other < naclac {
        format!("{other_label} -{}", naclac - other)
    } else {
        "equal".to_string()
    }
}

fn table(title: &str, naclac: &Outcome, other: &Outcome, pick: impl Fn(&Row) -> u64) {
    let other_label = other.framework.label();
    println!("\n{title}");
    println!(
        "  {:<16} {:>8} {:>8}   {}",
        "step",
        naclac.framework.label(),
        other_label,
        "cheaper"
    );
    let (mut naclac_total, mut other_total) = (0u64, 0u64);
    let (mut naclac_wins, mut other_wins) = (0usize, 0usize);
    for (n, o) in naclac.rows.iter().zip(&other.rows) {
        assert_eq!(n.step, o.step, "the two runs must send the same steps");
        let (n_cu, o_cu) = (pick(n), pick(o));
        naclac_total += n_cu;
        other_total += o_cu;
        naclac_wins += usize::from(n_cu < o_cu);
        other_wins += usize::from(o_cu < n_cu);
        println!(
            "  {:<16} {:>8} {:>8}   {}",
            n.step,
            n_cu,
            o_cu,
            verdict(n_cu, o_cu, other_label)
        );
    }
    println!(
        "  {:<16} {:>8} {:>8}   {}",
        "total",
        naclac_total,
        other_total,
        verdict(naclac_total, other_total, other_label)
    );
    println!("  steps cheaper: naclac {naclac_wins}, {other_label} {other_wins}");
}

fn compare_success(other: Framework) {
    let other_so = other_so_path(other);
    let keys = Keys::new();
    let naclac = run(Framework::Naclac, &keys, &other_so);
    let compared = run(other, &keys, &other_so);

    assert_eq!(
        naclac.facts,
        compared.facts,
        "the two programs must end in the same state"
    );

    table(
        "Compute units of the whole transaction (the system program's own cost included)",
        &naclac,
        &compared,
        |row| row.cu,
    );

    let label = other.label();
    let (naclac_size, other_size) = (file_size(&naclac_so_path()), file_size(&other_so));
    println!("\nBinary size");
    println!(
        "  naclac {naclac_size} bytes ({:.2} KB)\n  {label} {other_size} bytes ({:.2} KB)\n  {}",
        naclac_size as f64 / 1024.0,
        other_size as f64 / 1024.0,
        verdict(naclac_size, other_size, label)
    );
}

#[test]
#[ignore = "needs QUASAR_TREASURY_SO; run with --ignored"]
fn treasury_naclac_against_quasar() {
    compare_success(Framework::Quasar);
}

#[test]
#[ignore = "needs ANCHOR_TREASURY_SO; run with --ignored"]
fn treasury_naclac_against_anchor() {
    compare_success(Framework::Anchor);
}

struct FailureCase {
    name: &'static str,
    cu: u64,
    error: String,
}

fn failure_paths(framework: Framework, keys: &Keys, other_so: &str) -> Vec<FailureCase> {
    let run = Run::new(framework, keys, other_so);
    let program = run.program;
    let provider = &run.provider;
    let admin = &keys.admin;
    let member_a = &keys.member_a;
    let mut out = Vec::new();

    let (cu, error) = run.send_failing(
        build_create_treasury(
            provider,
            program,
            run.vault_bump,
            2,
            CreateTreasuryAccounts {
                admin: admin.address(),
                treasury: keys.recipient,
                vault: run.vault,
                system_program: Address::default(),
            },
        )
        .signer(admin),
    );
    out.push(FailureCase {
        name: "create_treasury, wrong treasury address",
        cu,
        error,
    });

    run.send(
        "create_treasury",
        build_create_treasury(
            provider,
            program,
            run.vault_bump,
            2,
            CreateTreasuryAccounts {
                admin: admin.address(),
                treasury: run.treasury,
                vault: run.vault,
                system_program: Address::default(),
            },
        )
        .signer(admin),
    );
    run.send(
        "add_member",
        build_add_member(
            provider,
            program,
            member_a.address(),
            AddMemberAccounts {
                admin: admin.address(),
                treasury: run.treasury,
            },
        )
        .signer(admin),
    );

    let (cu, error) = run.send_failing(
        build_deposit(
            provider,
            program,
            DEPOSIT,
            DepositAccounts {
                depositor: admin.address(),
                treasury: run.treasury,
                vault: keys.recipient,
                system_program: Address::default(),
            },
        )
        .signer(admin),
    );
    out.push(FailureCase {
        name: "deposit, wrong vault address",
        cu,
        error,
    });

    let (cu, error) = run.send_failing(
        build_init_note(
            provider,
            program,
            keys.note_one,
            InitNoteAccounts {
                authority: member_a.address(),
                note: keys.recipient,
                system_program: Address::default(),
            },
        )
        .signer(member_a),
    );
    out.push(FailureCase {
        name: "init_note, wrong note address",
        cu,
        error,
    });

    let (cu, error) = run.send_failing(
        build_propose(
            provider,
            program,
            0,
            keys.recipient,
            PAYOUT,
            LATER,
            ProposeAccounts {
                proposer: member_a.address(),
                treasury: run.treasury,
                proposal: keys.recipient,
                note: Some(Keypair::new().address()),
                system_program: Address::default(),
            },
        )
        .signer(member_a),
    );
    out.push(FailureCase {
        name: "propose, wrong proposal address, note present",
        cu,
        error,
    });

    for (name, proposal, amount) in [
        ("propose, wrong proposal address, note absent", keys.recipient, PAYOUT),
        ("propose, zero amount (fails in the handler)", run.proposal(), 0),
    ] {
        let (cu, error) = run.send_failing(
            build_propose(
                provider,
                program,
                0,
                keys.recipient,
                amount,
                LATER,
                ProposeAccounts {
                    proposer: member_a.address(),
                    treasury: run.treasury,
                    proposal,
                    note: None,
                    system_program: Address::default(),
                },
            )
            .signer(member_a),
        );
        out.push(FailureCase { name, cu, error });
    }

    out
}

#[test]
#[ignore = "needs QUASAR_TREASURY_SO; run with --ignored"]
fn treasury_failure_paths_naclac_against_quasar() {
    compare_failure_paths(Framework::Quasar);
}

#[test]
#[ignore = "needs ANCHOR_TREASURY_SO; run with --ignored"]
fn treasury_failure_paths_naclac_against_anchor() {
    compare_failure_paths(Framework::Anchor);
}

fn compare_failure_paths(other: Framework) {
    let other_so = other_so_path(other);
    let keys = Keys::new();
    let naclac = failure_paths(Framework::Naclac, &keys, &other_so);
    let compared = failure_paths(other, &keys, &other_so);
    let label = other.label();

    println!("\nCompute units the program consumed before the failure");
    println!("  {:<46} {:>8} {:>8}   cheaper", "case", "naclac", label);
    for (n, o) in naclac.iter().zip(&compared) {
        assert_eq!(n.name, o.name, "the two runs must send the same cases");
        println!(
            "  {:<46} {:>8} {:>8}   {}",
            n.name,
            n.cu,
            o.cu,
            verdict(n.cu, o.cu, label)
        );
        println!("    naclac error: {}", n.error);
        println!("    {label} error: {}", o.error);
    }
}
