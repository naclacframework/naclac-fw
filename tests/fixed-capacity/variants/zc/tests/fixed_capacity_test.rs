//! End-to-end LiteSVM tests for `FixedVec` and `FixedString` as account fields,
//! instruction arguments and fields of a defined type: every write goes
//! through the generated Rust client to the compiled program, and the account
//! bytes are compared with a layout built here from the rules alone (a
//! two-byte little-endian length, then every slot, no padding), not from the
//! program's or the client's own types. A stored or sent length above the
//! capacity, or a flag byte other than 0 or 1, must be rejected.

use fixed_capacity_client::{
    fetch_book, fetch_profile, get_book_pda, get_profile_pda, instructions::*,
    types::{Entry, Roster, PROGRAM_ID},
    Book, Profile, BOOK_DISCRIMINATOR, PROFILE_DISCRIMINATOR,
};
use naclac_client::*;

const INVALID_INSTRUCTION_DATA: u32 = 10_012;
const CAPACITY_EXCEEDED: u32 = 10_032;

type Outcome = Result<NaclacTransactionMetadata, NaclacClientError>;

fn assert_custom(result: Outcome, expected: u32) {
    match result {
        Err(NaclacClientError::TransactionFailed {
            instruction_err: InstructionError::Custom(code),
            ..
        }) => assert_eq!(code, expected, "wrong custom error code"),
        other => panic!("expected a Custom({expected}) transaction failure, got {other:?}"),
    }
}

fn assert_instruction_error(result: Outcome, expected: InstructionError) {
    match result {
        Err(NaclacClientError::TransactionFailed {
            instruction_err, ..
        }) => assert_eq!(instruction_err, expected),
        other => panic!("expected a {expected:?} transaction failure, got {other:?}"),
    }
}

fn setup() -> (NaclacProvider, Address) {
    let payer = load_node_wallet().expect("Failed to load local Solana keypair");
    let provider = NaclacProvider::new("litesvm", payer).expect("Failed to construct provider");
    let (profile, _bump) = get_profile_pda(&PROGRAM_ID);
    build_init_profile(
        &provider,
        PROGRAM_ID,
        InitProfileAccounts {
            payer: provider.payer.address(),
            profile,
            system_program: Address::default(),
        },
    )
    .send_and_confirm()
    .expect("init_profile should succeed");
    (provider, profile)
}

fn setup_book() -> (NaclacProvider, Address) {
    let payer = load_node_wallet().expect("Failed to load local Solana keypair");
    let provider = NaclacProvider::new("litesvm", payer).expect("Failed to construct provider");
    let (book, _bump) = get_book_pda(&PROGRAM_ID);
    build_init_book(
        &provider,
        PROGRAM_ID,
        InitBookAccounts {
            payer: provider.payer.address(),
            book,
            system_program: Address::default(),
        },
    )
    .send_and_confirm()
    .expect("init_book should succeed");
    (provider, book)
}

fn forge(provider: &NaclacProvider, address: &Address, edit: impl FnOnce(&mut Vec<u8>)) {
    let mut data = raw(provider, address);
    let owner = provider.get_account(address).unwrap().owner;
    let lamports = provider.get_balance(address).unwrap();
    edit(&mut data);
    provider.set_account(address, data, &owner, lamports).unwrap();
}

fn send_profile_data(provider: &NaclacProvider, profile: Address, data: Vec<u8>) -> Outcome {
    naclac_client::InstructionBuilder::new(provider, PROGRAM_ID, data)
        .account(
            naclac_client::AccountMeta::new_readonly(provider.payer.address(), true),
            "signer",
        )
        .account(naclac_client::AccountMeta::new(profile, false), "profile")
        .send_and_confirm()
}

fn send_book_data(provider: &NaclacProvider, book: Address, data: Vec<u8>) -> Outcome {
    naclac_client::InstructionBuilder::new(provider, PROGRAM_ID, data)
        .account(
            naclac_client::AccountMeta::new_readonly(provider.payer.address(), true),
            "signer",
        )
        .account(naclac_client::AccountMeta::new(book, false), "book")
        .send_and_confirm()
}

fn read(provider: &NaclacProvider, profile: &Address) -> Profile {
    fetch_profile(provider, profile).expect("profile should decode")
}

fn raw(provider: &NaclacProvider, profile: &Address) -> Vec<u8> {
    provider.get_account_data(profile).expect("account data")
}

fn push_score(provider: &NaclacProvider, profile: Address, value: u64) -> Outcome {
    build_push_score(
        provider,
        PROGRAM_ID,
        value,
        PushScoreAccounts {
            signer: provider.payer.address(),
            profile,
        },
    )
    .send_and_confirm()
}

/// A fresh blockhash, so a call repeated with the same arguments is not a
/// byte-identical transaction, which LiteSVM rejects as already processed.
fn fresh_blockhash(provider: &NaclacProvider) {
    provider
        .expire_blockhash()
        .expect("the litesvm backend can expire its blockhash");
}

fn pop_score(provider: &NaclacProvider, profile: Address) -> Outcome {
    fresh_blockhash(provider);
    build_pop_score(
        provider,
        PROGRAM_ID,
        PopScoreAccounts {
            signer: provider.payer.address(),
            profile,
        },
    )
    .send_and_confirm()
}

fn set_scores(provider: &NaclacProvider, profile: Address, values: Vec<u64>) -> Outcome {
    build_set_scores(
        provider,
        PROGRAM_ID,
        values,
        SetScoresAccounts {
            signer: provider.payer.address(),
            profile,
        },
    )
    .send_and_confirm()
}

fn set_name(provider: &NaclacProvider, profile: Address, name: &str) -> Outcome {
    build_set_name(
        provider,
        PROGRAM_ID,
        name.to_string(),
        SetNameAccounts {
            signer: provider.payer.address(),
            profile,
        },
    )
    .send_and_confirm()
}

fn add_member(provider: &NaclacProvider, profile: Address, member: Address) -> Outcome {
    build_add_member(
        provider,
        PROGRAM_ID,
        member,
        AddMemberAccounts {
            signer: provider.payer.address(),
            profile,
        },
    )
    .send_and_confirm()
}

fn remove_member(provider: &NaclacProvider, profile: Address, index: u8) -> Outcome {
    build_remove_member(
        provider,
        PROGRAM_ID,
        index,
        RemoveMemberAccounts {
            signer: provider.payer.address(),
            profile,
        },
    )
    .send_and_confirm()
}

fn push_flag(provider: &NaclacProvider, profile: Address, value: u8) -> Outcome {
    fresh_blockhash(provider);
    build_push_flag(
        provider,
        PROGRAM_ID,
        value,
        PushFlagAccounts {
            signer: provider.payer.address(),
            profile,
        },
    )
    .send_and_confirm()
}

fn bump_counter(provider: &NaclacProvider, profile: Address) -> Outcome {
    fresh_blockhash(provider);
    build_bump_counter(
        provider,
        PROGRAM_ID,
        BumpCounterAccounts {
            signer: provider.payer.address(),
            profile,
        },
    )
    .send_and_confirm()
}

fn clear_all(provider: &NaclacProvider, profile: Address) -> Outcome {
    fresh_blockhash(provider);
    build_clear_all(
        provider,
        PROGRAM_ID,
        ClearAllAccounts {
            signer: provider.payer.address(),
            profile,
        },
    )
    .send_and_confirm()
}

fn member(byte: u8) -> Address {
    Address::new_from_array([byte; 32])
}

fn scores_of(profile: &Profile) -> Vec<u64> {
    profile.scores.iter().collect()
}

/// A `FixedVec` or `FixedString` as the program stores it: a two-byte
/// little-endian length, then the items, then zeros up to `capacity_bytes`.
fn packed_list(len: usize, items: &[u8], capacity_bytes: usize) -> Vec<u8> {
    let mut out = (len as u16).to_le_bytes().to_vec();
    out.extend_from_slice(items);
    out.resize(2 + capacity_bytes, 0);
    out
}

fn offsets() -> (usize, usize, usize, usize) {
    let base = PROFILE_DISCRIMINATOR.len();
    (
        base + core::mem::offset_of!(Profile, scores),
        base + core::mem::offset_of!(Profile, name),
        base + core::mem::offset_of!(Profile, members),
        base + core::mem::offset_of!(Profile, flags),
    )
}

#[test]
fn the_account_is_packed_with_no_padding() {
    assert_eq!(core::mem::align_of::<Profile>(), 1);
    assert_eq!(
        core::mem::size_of::<Profile>(),
        1 + 32 + (2 + 4 * 8) + (2 + 16) + (2 + 3 * 32) + (2 + 8) + 2
    );
}

#[test]
fn a_new_account_has_empty_lists_and_the_payer_as_owner() {
    let (provider, profile) = setup();
    let state = read(&provider, &profile);
    assert!(state.scores.is_empty());
    assert!(state.name.is_empty());
    assert!(state.members.is_empty());
    assert!(state.flags.is_empty());
    assert_eq!(state.owner, provider.payer.address());
    let counter = state.counter;
    assert_eq!(counter, 0);
}

#[test]
fn a_list_fills_to_its_capacity_and_the_next_push_is_rejected() {
    let (provider, profile) = setup();
    for value in [11, 22, 33, 44] {
        push_score(&provider, profile, value).expect("push within capacity");
    }
    assert_eq!(scores_of(&read(&provider, &profile)), vec![11, 22, 33, 44]);

    assert_custom(push_score(&provider, profile, 55), CAPACITY_EXCEEDED);
    assert_eq!(scores_of(&read(&provider, &profile)), vec![11, 22, 33, 44]);
}

#[test]
fn popping_and_replacing_zero_the_freed_slots() {
    let (provider, profile) = setup();
    for value in [0x1111_1111_1111_1111, 0x2222_2222_2222_2222, 0x3333_3333_3333_3333] {
        push_score(&provider, profile, value).unwrap();
    }
    pop_score(&provider, profile).unwrap();
    let (scores_at, ..) = offsets();
    let bytes = raw(&provider, &profile);
    assert_eq!(&bytes[scores_at..scores_at + 2], &[2, 0]);
    assert!(bytes[scores_at + 2 + 16..scores_at + 2 + 32].iter().all(|b| *b == 0));

    set_scores(&provider, profile, vec![9]).unwrap();
    let bytes = raw(&provider, &profile);
    assert_eq!(&bytes[scores_at..scores_at + 2], &[1, 0]);
    assert_eq!(&bytes[scores_at + 2..scores_at + 10], &9u64.to_le_bytes());
    assert!(bytes[scores_at + 10..scores_at + 34].iter().all(|b| *b == 0));
}

#[test]
fn replacing_a_list_with_one_too_long_fails_and_changes_nothing() {
    let (provider, profile) = setup();
    set_scores(&provider, profile, vec![1, 2]).unwrap();
    assert_custom(
        set_scores(&provider, profile, vec![1, 2, 3, 4, 5]),
        CAPACITY_EXCEEDED,
    );
    assert_eq!(scores_of(&read(&provider, &profile)), vec![1, 2]);
}

#[test]
fn popping_an_empty_list_is_an_error() {
    let (provider, profile) = setup();
    assert_custom(pop_score(&provider, profile), INVALID_INSTRUCTION_DATA);
}

#[test]
fn a_string_is_stored_by_bytes_and_a_shorter_one_leaves_no_tail() {
    let (provider, profile) = setup();
    let (_, name_at, ..) = offsets();

    set_name(&provider, profile, "naclac").unwrap();
    let bytes = raw(&provider, &profile);
    assert_eq!(&bytes[name_at..name_at + 8], &[6, 0, b'n', b'a', b'c', b'l', b'a', b'c']);

    set_name(&provider, profile, "hi").unwrap();
    let bytes = raw(&provider, &profile);
    assert_eq!(&bytes[name_at..name_at + 4], &[2, 0, b'h', b'i']);
    assert!(bytes[name_at + 4..name_at + 18].iter().all(|b| *b == 0));
    assert_eq!(read(&provider, &profile).name.as_str().ok(), Some("hi"));
}

#[test]
fn capacity_counts_bytes_not_characters() {
    let (provider, profile) = setup();
    set_name(&provider, profile, &"é".repeat(8)).expect("16 bytes fit");
    assert_custom(set_name(&provider, profile, &"é".repeat(9)), CAPACITY_EXCEEDED);
    let expected = "é".repeat(8);
    assert_eq!(
        read(&provider, &profile).name.as_str().ok(),
        Some(expected.as_str())
    );
}

#[test]
fn invalid_utf8_in_a_string_argument_is_rejected() {
    let (provider, profile) = setup();
    let mut data = build_set_name(
        &provider,
        PROGRAM_ID,
        "ab".to_string(),
        SetNameAccounts {
            signer: provider.payer.address(),
            profile,
        },
    )
    .build_instruction()
    .data;
    // discriminator(8) + length(4) puts the first byte of "ab" at 12
    data[12] = 0xFF;
    let result = naclac_client::InstructionBuilder::new(&provider, PROGRAM_ID, data)
        .account(
            naclac_client::AccountMeta::new_readonly(provider.payer.address(), true),
            "signer",
        )
        .account(naclac_client::AccountMeta::new(profile, false), "profile")
        .send_and_confirm();
    assert_custom(result, INVALID_INSTRUCTION_DATA);
}

#[test]
fn invalid_utf8_stored_in_the_account_is_an_error_not_a_panic() {
    let (provider, profile) = setup();
    set_name(&provider, profile, "ok").unwrap();
    let (_, name_at, ..) = offsets();
    let mut data = raw(&provider, &profile);
    let owner = provider.get_account(&profile).unwrap().owner;
    let lamports = provider.get_balance(&profile).unwrap();
    data[name_at + 2] = 0xFF;
    provider.set_account(&profile, data, &owner, lamports).unwrap();

    let state = read(&provider, &profile);
    assert!(state.name.as_str().is_err());
    assert_eq!(state.name.len(), 2);
}

#[test]
fn a_list_of_addresses_removes_in_order_and_zeroes_the_freed_slot() {
    let (provider, profile) = setup();
    for byte in [1, 2, 3] {
        add_member(&provider, profile, member(byte)).unwrap();
    }
    assert_custom(add_member(&provider, profile, member(4)), CAPACITY_EXCEEDED);

    remove_member(&provider, profile, 0).unwrap();
    let state = read(&provider, &profile);
    assert_eq!(state.members.iter().collect::<Vec<_>>(), vec![member(2), member(3)]);

    let (.., members_at, _) = offsets();
    let bytes = raw(&provider, &profile);
    assert_eq!(&bytes[members_at..members_at + 2], &[2, 0]);
    assert!(bytes[members_at + 2 + 64..members_at + 2 + 96].iter().all(|b| *b == 0));

    assert_custom(remove_member(&provider, profile, 5), INVALID_INSTRUCTION_DATA);
}

#[test]
fn flags_are_stored_as_one_or_zero() {
    let (provider, profile) = setup();
    push_flag(&provider, profile, 2).unwrap();
    push_flag(&provider, profile, 0).unwrap();
    let (.., flags_at) = offsets();
    let bytes = raw(&provider, &profile);
    assert_eq!(&bytes[flags_at..flags_at + 4], &[2, 0, 1, 0]);
    let state = read(&provider, &profile);
    assert_eq!(
        state.flags.iter().map(|flag| flag.0).collect::<Vec<_>>(),
        vec![1, 0]
    );
}

#[test]
fn fixed_fields_do_not_disturb_the_fields_around_them() {
    let (provider, profile) = setup();
    bump_counter(&provider, profile).unwrap();
    for value in [1, 2, 3, 4] {
        push_score(&provider, profile, value).unwrap();
    }
    set_name(&provider, profile, "0123456789abcdef").unwrap();
    for byte in [1, 2, 3] {
        add_member(&provider, profile, member(byte)).unwrap();
    }
    for _ in 0..8 {
        push_flag(&provider, profile, 1).unwrap();
    }
    bump_counter(&provider, profile).unwrap();

    let state = read(&provider, &profile);
    assert_eq!(state.owner, provider.payer.address());
    let counter = state.counter;
    assert_eq!(counter, 2);

    clear_all(&provider, profile).unwrap();
    let state = read(&provider, &profile);
    assert!(state.scores.is_empty() && state.name.is_empty());
    assert!(state.members.is_empty() && state.flags.is_empty());
    let counter = state.counter;
    assert_eq!(counter, 2);
    let bytes = raw(&provider, &profile);
    let body = &bytes[PROFILE_DISCRIMINATOR.len()..];
    let lists_start = 1 + 32;
    let lists_end = body.len() - 2;
    let lists = &body[lists_start..lists_end];
    assert!(
        lists.iter().all(|b| *b == 0),
        "clear_all must leave every list byte zero"
    );
}

#[test]
fn a_forged_length_above_capacity_is_rejected_when_the_account_loads() {
    let (scores_at, name_at, members_at, flags_at) = offsets();
    for (label, at, forged) in [
        ("scores", scores_at, u16::MAX),
        ("name", name_at, 17),
        ("members", members_at, 4),
        ("flags", flags_at, 9),
    ] {
        let (provider, profile) = setup();
        forge(&provider, &profile, |data| {
            data[at..at + 2].copy_from_slice(&forged.to_le_bytes());
        });
        let forged_bytes = raw(&provider, &profile);

        assert_instruction_error(
            bump_counter(&provider, profile),
            InstructionError::InvalidAccountData,
        );
        assert_instruction_error(
            push_score(&provider, profile, 1),
            InstructionError::InvalidAccountData,
        );
        assert_eq!(
            raw(&provider, &profile),
            forged_bytes,
            "{label}: a rejected load must not change the account"
        );
    }
}

#[test]
fn a_length_equal_to_the_capacity_is_valid() {
    let (scores_at, ..) = offsets();
    let (provider, profile) = setup();
    forge(&provider, &profile, |data| {
        data[scores_at..scores_at + 2].copy_from_slice(&4u16.to_le_bytes());
    });
    assert_custom(push_score(&provider, profile, 1), CAPACITY_EXCEEDED);
}

#[test]
fn a_flag_byte_other_than_zero_or_one_is_rejected_only_inside_the_length() {
    let (.., flags_at) = offsets();
    let (provider, profile) = setup();
    push_flag(&provider, profile, 1).unwrap();

    forge(&provider, &profile, |data| data[flags_at + 2 + 3] = 2);
    bump_counter(&provider, profile).expect("an unused slot is not validated");

    forge(&provider, &profile, |data| data[flags_at + 2] = 2);
    assert_instruction_error(
        bump_counter(&provider, profile),
        InstructionError::InvalidAccountData,
    );
}

fn roster_arguments() -> (FixedVec<u64, 4>, FixedString<16>) {
    let mut scores = FixedVec::<u64, 4>::new();
    assert!(scores.try_push(7).is_ok());
    assert!(scores.try_push(8).is_ok());
    let mut name = FixedString::<16>::new();
    assert!(name.try_set("roster").is_ok());
    (scores, name)
}

#[test]
fn fixed_capacity_arguments_are_stored_and_a_forged_length_is_rejected() {
    let (provider, profile) = setup();
    let (scores, name) = roster_arguments();
    let accounts = || SetRosterAccounts {
        signer: provider.payer.address(),
        profile,
    };
    let data = build_set_roster(&provider, PROGRAM_ID, scores, name, accounts())
        .build_instruction()
        .data;
    build_set_roster(&provider, PROGRAM_ID, scores, name, accounts())
        .send_and_confirm()
        .expect("valid arguments are stored");

    let state = read(&provider, &profile);
    assert_eq!(scores_of(&state), vec![7, 8]);
    assert_eq!(state.name.as_str().ok(), Some("roster"));

    // discriminator(8) + scores(2 + 4 * 8) + name(2 + 16)
    for (label, at, forged) in [("scores", 8, 5u16), ("name", 8 + 34, 17u16)] {
        let mut forged_data = data.clone();
        forged_data[at..at + 2].copy_from_slice(&forged.to_le_bytes());
        fresh_blockhash(&provider);
        assert_custom(
            send_profile_data(&provider, profile, forged_data),
            INVALID_INSTRUCTION_DATA,
        );
        assert_eq!(scores_of(&read(&provider, &profile)), vec![7, 8], "{label}");
    }
}

#[test]
fn an_instruction_args_struct_with_fixed_capacity_fields_is_validated() {
    let (provider, profile) = setup();
    let (scores, name) = roster_arguments();
    let roster = Roster {
        scores,
        name,
        level: 3,
    };
    let accounts = || SetRosterGroupAccounts {
        signer: provider.payer.address(),
        profile,
    };
    let data = build_set_roster_group(&provider, PROGRAM_ID, roster, accounts())
        .build_instruction()
        .data;
    build_set_roster_group(&provider, PROGRAM_ID, roster, accounts())
        .send_and_confirm()
        .expect("a valid struct argument is stored");

    let state = read(&provider, &profile);
    assert_eq!(scores_of(&state), vec![7, 8]);
    assert_eq!(state.name.as_str().ok(), Some("roster"));
    let counter = state.counter;
    assert_eq!(counter, 3);

    let mut forged_data = data;
    forged_data[8..10].copy_from_slice(&5u16.to_le_bytes());
    fresh_blockhash(&provider);
    assert_custom(
        send_profile_data(&provider, profile, forged_data),
        INVALID_INSTRUCTION_DATA,
    );
}

#[test]
fn a_defined_type_with_fixed_capacity_fields_is_an_argument() {
    let (provider, book) = setup_book();
    let mut entry = Entry::default();
    assert!(entry.scores.try_push(5).is_ok());
    assert!(entry.name.try_set("first").is_ok());
    let accounts = || SetEntryAccounts {
        signer: provider.payer.address(),
        book,
    };
    let data = build_set_entry(&provider, PROGRAM_ID, entry, accounts())
        .build_instruction()
        .data;
    build_set_entry(&provider, PROGRAM_ID, entry, accounts())
        .send_and_confirm()
        .expect("a valid defined-type argument is stored");

    let state: Book = fetch_book(&provider, &book).expect("book should decode");
    let first = state.first;
    assert_eq!(first.scores.iter().collect::<Vec<u64>>(), vec![5]);
    assert_eq!(first.name.as_str().ok(), Some("first"));

    let mut forged_data = data;
    forged_data[8..10].copy_from_slice(&5u16.to_le_bytes());
    fresh_blockhash(&provider);
    assert_custom(
        send_book_data(&provider, book, forged_data),
        INVALID_INSTRUCTION_DATA,
    );
}

#[test]
fn a_defined_type_field_of_an_account_is_validated_when_the_account_loads() {
    let base = BOOK_DISCRIMINATOR.len();
    let first_scores = base + core::mem::offset_of!(Book, first) + core::mem::offset_of!(Entry, scores);
    let second_name = base
        + core::mem::offset_of!(Book, all)
        + core::mem::size_of::<Entry>()
        + core::mem::offset_of!(Entry, name);

    for (label, at, forged, valid) in [
        ("first.scores", first_scores, 5u16, false),
        ("all[1].name", second_name, 9, false),
        ("all[1].name at its capacity", second_name, 8, true),
    ] {
        let (provider, book) = setup_book();
        forge(&provider, &book, |data| {
            data[at..at + 2].copy_from_slice(&forged.to_le_bytes());
        });
        let result = build_touch_book(
            &provider,
            PROGRAM_ID,
            TouchBookAccounts {
                signer: provider.payer.address(),
                book,
            },
        )
        .send_and_confirm();
        if valid {
            result.unwrap_or_else(|error| panic!("{label}: expected success, got {error:?}"));
        } else {
            assert_instruction_error(result, InstructionError::InvalidAccountData);
        }
    }
}

#[test]
fn the_whole_account_matches_the_layout_built_from_the_rules() {
    let (provider, profile) = setup();
    push_score(&provider, profile, 10).unwrap();
    push_score(&provider, profile, 20).unwrap();
    set_name(&provider, profile, "naclac").unwrap();
    add_member(&provider, profile, member(7)).unwrap();
    push_flag(&provider, profile, 1).unwrap();
    push_flag(&provider, profile, 0).unwrap();
    bump_counter(&provider, profile).unwrap();
    bump_counter(&provider, profile).unwrap();

    let mut expected = vec![0u8];
    expected.extend_from_slice(&[0u8; 32]);
    expected.extend(packed_list(
        2,
        &[10u64.to_le_bytes(), 20u64.to_le_bytes()].concat(),
        4 * 8,
    ));
    expected.extend(packed_list(6, b"naclac", 16));
    expected.extend(packed_list(1, &[7u8; 32], 3 * 32));
    expected.extend(packed_list(2, &[1, 0], 8));
    expected.extend_from_slice(&2u16.to_le_bytes());

    let bytes = raw(&provider, &profile);
    let mut body = bytes[PROFILE_DISCRIMINATOR.len()..].to_vec();
    body[0] = 0;
    body[1..33].fill(0);
    assert_eq!(body.len(), expected.len());
    assert_eq!(hex(&body), hex(&expected));
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}
