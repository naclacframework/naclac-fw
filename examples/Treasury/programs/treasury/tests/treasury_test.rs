//! Integration tests for the treasury program through its generated client:
//! each instruction, and the failures it is meant to reject.

use {
    naclac_client::*,
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
        types::{
            Deposited, ProgramError as TreasuryError, ProposalExecuted, Status, TreasuryCreated,
            PROGRAM_ID, SEED_VAULT,
        },
    },
};

type Sent = Result<NaclacTransactionMetadata, NaclacClientError>;

const NOW: i64 = 1_000_000;
const LATER: i64 = NOW + 3_600;
const DEPOSIT: u64 = 5_000_000;
const PAYOUT: u64 = 1_000_000;

const UNAUTHORIZED: u32 = 21;
const CONSTRAINT_SEEDS: u32 = 6;
const ACCOUNT_DATA_TOO_SMALL: u32 = 10;

fn framework_code(field_index: u32, kind: u32) -> u32 {
    10_000 + field_index * 100 + kind
}

fn assert_code(result: Sent, expected: u32) {
    match result {
        Err(NaclacClientError::TransactionFailed {
            instruction_err: InstructionError::Custom(code),
            ..
        }) => assert_eq!(code, expected, "wrong custom error code"),
        other => panic!("expected Custom({expected}), got {other:?}"),
    }
}

fn assert_treasury_error(result: Sent, expected: TreasuryError) {
    match result {
        Err(NaclacClientError::TransactionFailed {
            instruction_err: InstructionError::Custom(code),
            ..
        }) => assert_eq!(
            TreasuryError::from_code(code),
            Some(expected),
            "wrong custom error code {code}"
        ),
        other => panic!("expected {expected:?}, got {other:?}"),
    }
}

fn send(provider: &NaclacProvider, builder: InstructionBuilder<'_>) -> Sent {
    provider
        .expire_blockhash()
        .expect("Failed to expire the blockhash");
    builder.send_and_confirm()
}

fn new_provider() -> NaclacProvider {
    let payer = load_node_wallet().expect("Failed to load local Solana keypair");
    let provider =
        NaclacProvider::new("litesvm", payer).expect("Failed to construct NaclacProvider");
    provider
        .set_clock_unix_timestamp(NOW)
        .expect("Failed to set the clock");
    provider
}

fn treasury_addresses(provider: &NaclacProvider) -> (Address, Address, u8) {
    let (treasury, _) = get_treasury_pda(&PROGRAM_ID, &provider.payer.address());
    let (vault, vault_bump) = get_vault_pda(&PROGRAM_ID, &treasury);
    (treasury, vault, vault_bump)
}

fn send_create_treasury(
    provider: &NaclacProvider,
    treasury: Address,
    vault: Address,
    vault_bump: u8,
    threshold: u8,
) -> Sent {
    build_create_treasury(
        provider,
        PROGRAM_ID,
        vault_bump,
        threshold,
        CreateTreasuryAccounts {
            admin: provider.payer.address(),
            treasury,
            vault,
            system_program: Address::default(),
        },
    )
    .send_and_confirm()
}

struct Fixture {
    provider: NaclacProvider,
    treasury: Address,
    vault: Address,
}

fn fixture(threshold: u8) -> Fixture {
    let provider = new_provider();
    let (treasury, vault, vault_bump) = treasury_addresses(&provider);
    send_create_treasury(&provider, treasury, vault, vault_bump, threshold)
        .expect("create_treasury should succeed");
    Fixture {
        provider,
        treasury,
        vault,
    }
}

impl Fixture {
    fn admin(&self) -> Address {
        self.provider.payer.address()
    }

    fn funded_keypair(&self) -> Keypair {
        let keypair = Keypair::new();
        transfer_sol(&self.provider, &keypair.address(), 1_000_000_000)
            .expect("Failed to fund a keypair");
        keypair
    }

    fn add_member(&self, member: &Address) -> Sent {
        send(
            &self.provider,
            build_add_member(
                &self.provider,
                PROGRAM_ID,
                *member,
                AddMemberAccounts {
                    admin: self.admin(),
                    treasury: self.treasury,
                },
            ),
        )
    }

    fn member(&self) -> Keypair {
        let keypair = self.funded_keypair();
        self.add_member(&keypair.address())
            .expect("add_member should succeed");
        keypair
    }

    fn deposit(&self, amount: u64) -> Sent {
        send(
            &self.provider,
            build_deposit(
                &self.provider,
                PROGRAM_ID,
                amount,
                DepositAccounts {
                    depositor: self.admin(),
                    treasury: self.treasury,
                    vault: self.vault,
                    system_program: Address::default(),
                },
            ),
        )
    }

    fn proposal_address(&self, id: u64) -> Address {
        get_proposal_pda(&PROGRAM_ID, &self.treasury, id).0
    }

    fn propose(
        &self,
        proposer: &Keypair,
        id: u64,
        recipient: Address,
        amount: u64,
        deadline: i64,
        note: Option<Address>,
    ) -> Sent {
        send(
            &self.provider,
            build_propose(
                &self.provider,
                PROGRAM_ID,
                id,
                recipient,
                amount,
                deadline,
                ProposeAccounts {
                    proposer: proposer.address(),
                    treasury: self.treasury,
                    proposal: self.proposal_address(id),
                    note,
                    system_program: Address::default(),
                },
            )
            .signer(proposer),
        )
    }

    fn approve(&self, id: u64, signers: &[&Keypair]) -> Sent {
        let metas = signers
            .iter()
            .map(|signer| AccountMeta::new_readonly(signer.address(), true))
            .collect();
        let mut builder = build_approve(
            &self.provider,
            PROGRAM_ID,
            ApproveAccounts {
                treasury: self.treasury,
                proposal: self.proposal_address(id),
            },
        )
        .remaining_accounts(metas);
        for signer in signers {
            builder = builder.signer(signer);
        }
        send(&self.provider, builder)
    }

    fn execute(&self, id: u64, recipient: Address) -> Sent {
        send(
            &self.provider,
            build_execute(
                &self.provider,
                PROGRAM_ID,
                ExecuteAccounts {
                    executor: self.admin(),
                    treasury: self.treasury,
                    proposal: self.proposal_address(id),
                    vault: self.vault,
                    recipient,
                    system_program: Address::default(),
                    event_authority: get_event_authority_pda(&PROGRAM_ID).0,
                    program: PROGRAM_ID,
                },
            ),
        )
    }

    fn init_note(&self, authority: &Keypair, id: u64) -> Address {
        let (note, _) = get_note_pda(&PROGRAM_ID, &authority.address(), id);
        build_init_note(
            &self.provider,
            PROGRAM_ID,
            id,
            InitNoteAccounts {
                authority: authority.address(),
                note,
                system_program: Address::default(),
            },
        )
        .signer(authority)
        .send_and_confirm()
        .expect("init_note should succeed");
        note
    }

    fn lamports(&self, address: &Address) -> u64 {
        self.provider
            .get_account(address)
            .expect("the account should exist")
            .lamports
    }

    fn funded_proposal(&self, signers: &[&Keypair], id: u64, recipient: Address) {
        self.deposit(DEPOSIT).expect("deposit should succeed");
        self.propose(signers[0], id, recipient, PAYOUT, LATER, None)
            .expect("propose should succeed");
        self.approve(id, signers).expect("approve should succeed");
    }
}

#[test]
fn create_treasury_records_its_fields_and_emits_its_event() {
    let provider = new_provider();
    let (treasury, vault, vault_bump) = treasury_addresses(&provider);
    let meta = send_create_treasury(&provider, treasury, vault, vault_bump, 2)
        .expect("create_treasury should succeed");

    let state = fetch_treasury(&provider, &treasury).expect("treasury should be readable");
    let (_, expected_bump) = get_treasury_pda(&PROGRAM_ID, &provider.payer.address());
    let (bump, stored_vault_bump, threshold) = (state.bump, state.vault_bump, state.threshold);
    let (next_id, total_deposited) = (state.next_id, state.total_deposited);
    assert_eq!(state.admin, provider.payer.address());
    assert_eq!(bump, expected_bump);
    assert_eq!(stored_vault_bump, vault_bump);
    assert_eq!(threshold, 2);
    assert_eq!(next_id, 0);
    assert_eq!(total_deposited, 0);
    assert!(matches!(state.status.get(), Ok(Status::Active)));
    assert_eq!(state.members.len(), 0);

    let events = meta
        .parse_events_zero_copy::<TreasuryCreated>()
        .expect("the event should decode");
    assert_eq!(events.len(), 1);
    let event_threshold = events[0].threshold;
    assert_eq!(events[0].admin, provider.payer.address());
    assert_eq!(event_threshold, 2);
}

#[test]
fn create_treasury_rejects_a_threshold_outside_one_to_five() {
    let provider = new_provider();
    let (treasury, vault, vault_bump) = treasury_addresses(&provider);
    for threshold in [0u8, 6] {
        let result = send_create_treasury(&provider, treasury, vault, vault_bump, threshold);
        assert_treasury_error(result, TreasuryError::InvalidThreshold);
    }
}

#[test]
fn create_treasury_rejects_a_vault_bump_that_is_not_the_canonical_one() {
    let provider = new_provider();
    let (treasury, _, canonical_bump) = treasury_addresses(&provider);
    let (other_bump, other_vault) = (0..canonical_bump)
        .rev()
        .find_map(|bump| {
            Address::create_program_address(&[SEED_VAULT, treasury.as_ref(), &[bump]], &PROGRAM_ID)
                .ok()
                .map(|address| (bump, address))
        })
        .expect("a non-canonical bump that derives an address exists");

    let result = send_create_treasury(&provider, treasury, other_vault, other_bump, 2);
    assert_code(result, framework_code(2, CONSTRAINT_SEEDS));
}

#[test]
fn create_treasury_rejects_a_vault_bump_argument_that_is_not_the_found_one() {
    let provider = new_provider();
    let (treasury, vault, canonical_bump) = treasury_addresses(&provider);

    let result = send_create_treasury(&provider, treasury, vault, canonical_bump.wrapping_sub(1), 2);
    assert_code(result, framework_code(2, CONSTRAINT_SEEDS));
}

#[test]
fn only_the_admin_adds_members_and_the_list_has_five_slots() {
    let fixture = fixture(1);
    let first = fixture.member();

    assert_treasury_error(
        fixture.add_member(&first.address()),
        TreasuryError::AlreadyMember,
    );
    for _ in 0..4 {
        fixture.member();
    }
    assert_treasury_error(
        fixture.add_member(&Keypair::new().address()),
        TreasuryError::MembersFull,
    );

    let state = fetch_treasury(&fixture.provider, &fixture.treasury)
        .expect("treasury should be readable");
    assert_eq!(state.members.len(), 5);
    assert!(state.members.contains(&first.address()));

    let stranger = Keypair::new();
    let result = build_add_member(
        &fixture.provider,
        PROGRAM_ID,
        Keypair::new().address(),
        AddMemberAccounts {
            admin: stranger.address(),
            treasury: fixture.treasury,
        },
    )
    .signer(&stranger)
    .send_and_confirm();
    assert_code(result, framework_code(1, UNAUTHORIZED));
}

#[test]
fn members_can_be_removed_but_never_below_the_threshold() {
    let fixture = fixture(2);
    let first = fixture.member();
    let second = fixture.member();

    let remove = |member: Address| {
        send(
            &fixture.provider,
            build_remove_member(
                &fixture.provider,
                PROGRAM_ID,
                member,
                RemoveMemberAccounts {
                    admin: fixture.admin(),
                    treasury: fixture.treasury,
                },
            ),
        )
    };

    assert_treasury_error(remove(Keypair::new().address()), TreasuryError::NotAMember);
    assert_treasury_error(remove(first.address()), TreasuryError::BelowThreshold);

    fixture.member();
    remove(first.address()).expect("remove_member should succeed");

    let state = fetch_treasury(&fixture.provider, &fixture.treasury)
        .expect("treasury should be readable");
    assert_eq!(state.members.len(), 2);
    assert!(!state.members.contains(&first.address()));
    assert!(state.members.contains(&second.address()));
}

#[test]
fn pausing_blocks_deposits_and_the_same_status_is_rejected() {
    let fixture = fixture(1);
    let set_status = |status: Status| {
        send(
            &fixture.provider,
            build_set_status(
                &fixture.provider,
                PROGRAM_ID,
                status,
                SetStatusAccounts {
                    admin: fixture.admin(),
                    treasury: fixture.treasury,
                },
            ),
        )
    };

    assert_treasury_error(set_status(Status::Active), TreasuryError::SameStatus);
    set_status(Status::Paused).expect("pausing should succeed");
    let state = fetch_treasury(&fixture.provider, &fixture.treasury)
        .expect("treasury should be readable");
    assert!(matches!(state.status.get(), Ok(Status::Paused)));

    assert_treasury_error(fixture.deposit(DEPOSIT), TreasuryError::Paused);
    set_status(Status::Active).expect("resuming should succeed");
    fixture.deposit(DEPOSIT).expect("deposit should succeed");
}

#[test]
fn deposit_moves_lamports_into_the_vault_and_counts_them() {
    let fixture = fixture(1);
    assert_treasury_error(fixture.deposit(0), TreasuryError::ZeroAmount);

    let meta = fixture.deposit(DEPOSIT).expect("deposit should succeed");
    assert_eq!(fixture.lamports(&fixture.vault), DEPOSIT);
    let state = fetch_treasury(&fixture.provider, &fixture.treasury)
        .expect("treasury should be readable");
    let total_deposited = state.total_deposited;
    assert_eq!(total_deposited, DEPOSIT);

    let events = meta
        .parse_events_zero_copy::<Deposited>()
        .expect("the event should decode");
    assert_eq!(events.len(), 1);
    let amount = events[0].amount;
    assert_eq!(amount, DEPOSIT);

    let result = build_deposit(
        &fixture.provider,
        PROGRAM_ID,
        DEPOSIT,
        DepositAccounts {
            depositor: fixture.admin(),
            treasury: fixture.treasury,
            vault: Keypair::new().address(),
            system_program: Address::default(),
        },
    )
    .send_and_confirm();
    assert_code(result, framework_code(2, CONSTRAINT_SEEDS));
}

#[test]
fn propose_records_a_proposal_and_advances_the_counter() {
    let fixture = fixture(1);
    let member = fixture.member();
    let recipient = Keypair::new().address();

    fixture
        .propose(&member, 0, recipient, PAYOUT, LATER, None)
        .expect("propose should succeed");

    let proposal = fetch_proposal(&fixture.provider, &fixture.proposal_address(0))
        .expect("proposal should be readable");
    let (id, amount, deadline) = (proposal.id, proposal.amount, proposal.deadline);
    assert_eq!(proposal.treasury, fixture.treasury);
    assert_eq!(proposal.proposer, member.address());
    assert_eq!(proposal.recipient, recipient);
    assert_eq!((id, amount, deadline), (0, PAYOUT, LATER));
    assert!(!bool::from(proposal.executed));
    assert_eq!(proposal.approvals.len(), 0);

    let state = fetch_treasury(&fixture.provider, &fixture.treasury)
        .expect("treasury should be readable");
    let next_id = state.next_id;
    assert_eq!(next_id, 1);
}

#[test]
fn propose_rejects_a_wrong_id_a_stranger_a_zero_amount_and_a_past_deadline() {
    let fixture = fixture(1);
    let member = fixture.member();
    let stranger = fixture.funded_keypair();
    let recipient = Keypair::new().address();

    assert_treasury_error(
        fixture.propose(&member, 5, recipient, PAYOUT, LATER, None),
        TreasuryError::InvalidProposalId,
    );
    assert_treasury_error(
        fixture.propose(&stranger, 0, recipient, PAYOUT, LATER, None),
        TreasuryError::NotAMember,
    );
    assert_treasury_error(
        fixture.propose(&member, 0, recipient, 0, LATER, None),
        TreasuryError::ZeroAmount,
    );
    assert_treasury_error(
        fixture.propose(&member, 0, recipient, PAYOUT, NOW, None),
        TreasuryError::DeadlinePassed,
    );
}

#[test]
fn propose_counts_the_proposers_note_and_rejects_someone_elses() {
    let fixture = fixture(1);
    let member = fixture.member();
    let other = fixture.member();
    let recipient = Keypair::new().address();
    let note = fixture.init_note(&member, 1);
    let others_note = fixture.init_note(&other, 1);

    fixture
        .propose(&member, 0, recipient, PAYOUT, LATER, Some(note))
        .expect("propose with a note should succeed");
    fixture
        .propose(&member, 1, recipient, PAYOUT, LATER, Some(note))
        .expect("propose with a note should succeed");
    let count = fetch_note(&fixture.provider, &note)
        .expect("note should be readable")
        .count;
    assert_eq!(count, 2);

    assert_treasury_error(
        fixture.propose(&member, 2, recipient, PAYOUT, LATER, Some(others_note)),
        TreasuryError::NotNoteAuthority,
    );
    fixture
        .propose(&member, 2, recipient, PAYOUT, LATER, None)
        .expect("propose without a note should succeed");
}

#[test]
fn approve_records_signing_members_once_each() {
    let fixture = fixture(2);
    let first = fixture.member();
    let second = fixture.member();
    let stranger = fixture.funded_keypair();
    fixture
        .propose(&first, 0, Keypair::new().address(), PAYOUT, LATER, None)
        .expect("propose should succeed");

    fixture
        .approve(0, &[&first])
        .expect("approve should succeed");
    assert_treasury_error(
        fixture.approve(0, &[&first]),
        TreasuryError::AlreadyApproved,
    );
    assert_treasury_error(fixture.approve(0, &[&stranger]), TreasuryError::NotAMember);

    let unsigned = build_approve(
        &fixture.provider,
        PROGRAM_ID,
        ApproveAccounts {
            treasury: fixture.treasury,
            proposal: fixture.proposal_address(0),
        },
    )
    .remaining_accounts(vec![AccountMeta::new_readonly(second.address(), false)])
    .send_and_confirm();
    assert!(matches!(
        unsigned,
        Err(NaclacClientError::TransactionFailed {
            instruction_err: InstructionError::MissingRequiredSignature,
            ..
        })
    ));

    fixture
        .approve(0, &[&second])
        .expect("approve should succeed");
    let proposal = fetch_proposal(&fixture.provider, &fixture.proposal_address(0))
        .expect("proposal should be readable");
    assert_eq!(proposal.approvals.len(), 2);
}

#[test]
fn execute_pays_the_recipient_returns_the_vault_balance_and_emits_the_event() {
    let fixture = fixture(2);
    let first = fixture.member();
    let second = fixture.member();
    let recipient = Keypair::new().address();
    fixture.funded_proposal(&[&first, &second], 0, recipient);

    let meta = fixture
        .execute(0, recipient)
        .expect("execute should succeed");

    assert_eq!(fixture.lamports(&recipient), PAYOUT);
    assert_eq!(fixture.lamports(&fixture.vault), DEPOSIT - PAYOUT);
    let returned = u64::from_le_bytes(
        meta.return_data[..8]
            .try_into()
            .expect("the return data is a u64"),
    );
    assert_eq!(returned, DEPOSIT - PAYOUT);

    let events = meta
        .parse_events_zero_copy::<ProposalExecuted>()
        .expect("the event should decode");
    assert_eq!(events.len(), 1);
    let (id, amount) = (events[0].id, events[0].amount);
    assert_eq!((id, amount), (0, PAYOUT));
    assert_eq!(events[0].recipient, recipient);

    let proposal = fetch_proposal(&fixture.provider, &fixture.proposal_address(0))
        .expect("proposal should be readable");
    assert!(bool::from(proposal.executed));
    let state = fetch_treasury(&fixture.provider, &fixture.treasury)
        .expect("treasury should be readable");
    let total_deposited = state.total_deposited;
    assert_eq!(total_deposited, DEPOSIT - PAYOUT);

    assert_treasury_error(
        fixture.execute(0, recipient),
        TreasuryError::AlreadyExecuted,
    );
    assert_treasury_error(
        fixture.approve(0, &[&first]),
        TreasuryError::AlreadyExecuted,
    );
}

#[test]
fn execute_rejects_what_the_proposal_has_not_earned() {
    let fixture = fixture(2);
    let first = fixture.member();
    let second = fixture.member();
    let recipient = Keypair::new().address();
    fixture.deposit(DEPOSIT).expect("deposit should succeed");
    fixture
        .propose(&first, 0, recipient, PAYOUT, LATER, None)
        .expect("propose should succeed");

    fixture
        .approve(0, &[&first])
        .expect("approve should succeed");
    assert_treasury_error(
        fixture.execute(0, recipient),
        TreasuryError::NotEnoughApprovals,
    );

    fixture
        .approve(0, &[&second])
        .expect("approve should succeed");
    assert_code(
        fixture.execute(0, Keypair::new().address()),
        framework_code(2, UNAUTHORIZED),
    );

    fixture
        .provider
        .set_clock_unix_timestamp(LATER + 1)
        .expect("Failed to set the clock");
    assert_treasury_error(
        fixture.execute(0, recipient),
        TreasuryError::DeadlinePassed,
    );
}

#[test]
fn execute_rejects_a_proposal_the_vault_cannot_cover() {
    let fixture = fixture(1);
    let member = fixture.member();
    let recipient = Keypair::new().address();
    fixture
        .deposit(PAYOUT)
        .expect("deposit should succeed");
    fixture
        .propose(&member, 0, recipient, PAYOUT * 2, LATER, None)
        .expect("propose should succeed");
    fixture
        .approve(0, &[&member])
        .expect("approve should succeed");

    assert_treasury_error(
        fixture.execute(0, recipient),
        TreasuryError::InsufficientFunds,
    );
}

#[test]
fn a_paused_treasury_does_not_execute() {
    let fixture = fixture(1);
    let member = fixture.member();
    let recipient = Keypair::new().address();
    fixture.funded_proposal(&[&member], 0, recipient);

    build_set_status(
        &fixture.provider,
        PROGRAM_ID,
        Status::Paused,
        SetStatusAccounts {
            admin: fixture.admin(),
            treasury: fixture.treasury,
        },
    )
    .send_and_confirm()
    .expect("pausing should succeed");

    assert_treasury_error(fixture.execute(0, recipient), TreasuryError::Paused);
}

#[test]
fn close_proposal_refunds_the_proposer_once_the_proposal_is_executed() {
    let fixture = fixture(1);
    let member = fixture.member();
    let stranger = fixture.funded_keypair();
    let recipient = Keypair::new().address();
    fixture.funded_proposal(&[&member], 0, recipient);

    let close = |proposer: &Keypair| {
        send(
            &fixture.provider,
            build_close_proposal(
                &fixture.provider,
                PROGRAM_ID,
                CloseProposalAccounts {
                    proposer: proposer.address(),
                    proposal: fixture.proposal_address(0),
                },
            )
            .signer(proposer),
        )
    };

    assert_treasury_error(close(&member), TreasuryError::NotExecuted);
    fixture
        .execute(0, recipient)
        .expect("execute should succeed");
    assert_code(close(&stranger), framework_code(1, UNAUTHORIZED));

    let before = fixture.lamports(&member.address());
    close(&member).expect("close_proposal should succeed");
    assert!(fixture.lamports(&member.address()) > before);
    assert!(fixture
        .provider
        .get_account(&fixture.proposal_address(0))
        .is_err());
}

#[test]
fn a_note_can_grow_but_not_shrink_below_its_own_size() {
    let fixture = fixture(1);
    let authority = fixture.funded_keypair();
    let note = fixture.init_note(&authority, 1);

    let resize = |new_space: u64| {
        build_resize_note(
            &fixture.provider,
            PROGRAM_ID,
            new_space,
            ResizeNoteAccounts {
                authority: authority.address(),
                note,
                system_program: Address::default(),
            },
        )
        .signer(&authority)
        .send_and_confirm()
    };

    resize(100).expect("growing should succeed");
    let grown = fixture
        .provider
        .get_account(&note)
        .expect("note should exist");
    assert_eq!(grown.data.len(), 100);

    assert_code(resize(1), framework_code(1, ACCOUNT_DATA_TOO_SMALL));
    let unchanged = fixture
        .provider
        .get_account(&note)
        .expect("note should exist");
    assert_eq!(unchanged.data.len(), 100);
}

#[test]
fn merge_notes_moves_a_count_and_rejects_the_same_note_twice() {
    let fixture = fixture(1);
    let member = fixture.member();
    let recipient = Keypair::new().address();
    let from = fixture.init_note(&member, 1);
    let to = fixture.init_note(&member, 2);
    for id in 0..2 {
        fixture
            .propose(&member, id, recipient, PAYOUT, LATER, Some(from))
            .expect("propose with a note should succeed");
    }

    let merge = |from: Address, to: Address, amount: u64| {
        build_merge_notes(
            &fixture.provider,
            PROGRAM_ID,
            amount,
            MergeNotesAccounts {
                authority: member.address(),
                from,
                to,
            },
        )
        .signer(&member)
        .send_and_confirm()
    };

    assert_treasury_error(merge(from, to, 3), TreasuryError::InsufficientCount);
    merge(from, to, 1).expect("merge_notes should succeed");
    let moved_from = fetch_note(&fixture.provider, &from)
        .expect("note should be readable")
        .count;
    let moved_to = fetch_note(&fixture.provider, &to)
        .expect("note should be readable")
        .count;
    assert_eq!((moved_from, moved_to), (1, 1));

    assert!(merge(from, from, 1).is_err());
    let unchanged = fetch_note(&fixture.provider, &from)
        .expect("note should be readable")
        .count;
    assert_eq!(unchanged, 1);
}
