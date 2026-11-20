//! End-to-end LiteSVM tests of the zero-copy family: every instruction goes
//! through the generated Rust client to the compiled program, accounts are
//! read back and events are decoded from the transaction metadata.

use naclac_client::*;
use types_lab_zc_client::{
    fetch_book, fetch_lab, fetch_note, get_book_pda, get_lab_pda, get_note_pda,
    instructions::*,
    types::{
        Action, Color, Entry, EnumsChanged, LabLog, Level, Limits, NestedArgs, OptionArgs,
        RosterChanged, ScalarArgs, ScalarsSet, Signed, TextLog, Wide, PROGRAM_ID,
    },
    Lab, LAB_DISCRIMINATOR,
};

const INVALID_INSTRUCTION_DATA: u32 = 10_012;
const CAPACITY_EXCEEDED: u32 = 10_032;
const REJECTED: u32 = 6000;
const OVERFLOWED: u32 = 6001;
const ROSTER_FULL: u32 = 6002;

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

fn fresh_blockhash(provider: &NaclacProvider) {
    provider
        .expire_blockhash()
        .expect("the litesvm backend can expire its blockhash");
}

fn setup() -> (NaclacProvider, Address) {
    let payer = load_node_wallet().expect("Failed to load local Solana keypair");
    let provider = NaclacProvider::new("litesvm", payer).expect("Failed to construct provider");
    let (lab, _bump) = get_lab_pda(&PROGRAM_ID);
    build_init_lab(
        &provider,
        PROGRAM_ID,
        InitLabAccounts {
            payer: provider.payer.address(),
            lab,
            system_program: Address::default(),
        },
    )
    .send_and_confirm()
    .expect("init_lab should succeed");
    (provider, lab)
}

macro_rules! send {
    ($provider:expr, $lab:expr, $build:ident, $accounts:ident $(, $arg:expr)*) => {{
        fresh_blockhash($provider);
        $build(
            $provider,
            PROGRAM_ID
            $(, $arg)*,
            $accounts {
                signer: $provider.payer.address(),
                lab: $lab,
            },
        )
        .send_and_confirm()
    }};
}

fn read(provider: &NaclacProvider, lab: &Address) -> Lab {
    fetch_lab(provider, lab).expect("lab should decode")
}

fn scalar_args() -> ScalarArgs {
    ScalarArgs {
        flag: Bool::from(true),
        small: 200,
        short: 60_000,
        medium: 4_000_000_000,
        large: u64::MAX - 5,
        huge: u128::MAX - 7,
        tiny_signed: -100,
        short_signed: -30_000,
        medium_signed: -2_000_000_000,
        large_signed: i64::MIN + 3,
        huge_signed: i128::MIN + 11,
        single: 1.5,
        double: -2.25,
        key: Address::new_from_array([4; 32]),
    }
}

#[test]
fn init_writes_the_chosen_variants_and_zero_elsewhere() {
    let (provider, lab) = setup();
    let state = read(&provider, &lab);
    assert_eq!(state.authority, provider.payer.address());
    assert!(matches!(state.color.get(), Ok(Color::Green)));
    assert!(matches!(state.action.get(), Ok(Action::SetFee { fee_bps: 25 })));
    assert!(matches!(state.level.get(), Ok(Level::Mid)));
    assert!(matches!(state.signed.get(), Ok(Signed::Neg)));
    assert!(matches!(state.wide.get(), Ok(Wide::Small(5))));
    assert!(matches!(state.history[0].get(), Ok(Color::Blue)));
    assert!(matches!(state.history[1].get(), Ok(Color::Red)));
    assert!(state.maybe_u64.get().is_none());
    assert!(state.scores.is_empty() && state.members.is_empty() && state.flags.is_empty());
    assert!(state.name.is_empty());
    let counter = state.counter;
    assert_eq!(counter, 0);
}

#[test]
fn every_scalar_type_round_trips_through_the_account_and_the_event() {
    let (provider, lab) = setup();
    let meta = send!(&provider, lab, build_set_scalars, SetScalarsAccounts, scalar_args())
        .expect("set_scalars should succeed");

    let state = read(&provider, &lab);
    let flag = state.flag.0;
    let (small, short, medium, large, huge) =
        (state.small, state.short, state.medium, state.large, state.huge);
    let (tiny, short_s, medium_s, large_s, huge_s) = (
        state.tiny_signed,
        state.short_signed,
        state.medium_signed,
        state.large_signed,
        state.huge_signed,
    );
    let (single, double, key) = (state.single, state.double, state.key);
    assert_eq!(flag, 1);
    assert_eq!((small, short, medium), (200, 60_000, 4_000_000_000));
    assert_eq!((large, huge), (u64::MAX - 5, u128::MAX - 7));
    assert_eq!((tiny, short_s, medium_s), (-100, -30_000, -2_000_000_000));
    assert_eq!((large_s, huge_s), (i64::MIN + 3, i128::MIN + 11));
    assert_eq!((single, double), (1.5, -2.25));
    assert_eq!(key, Address::new_from_array([4; 32]));

    let events = meta
        .parse_events_zero_copy::<ScalarsSet>()
        .expect("events should parse");
    assert_eq!(events.len(), 1);
    let event = events[0];
    let (e_flag, e_small, e_huge, e_huge_s) =
        (event.flag.0, event.small, event.huge, event.huge_signed);
    let (e_single, e_double) = (event.single, event.double);
    assert_eq!((e_flag, e_small), (1, 200));
    assert_eq!((e_huge, e_huge_s), (u128::MAX - 7, i128::MIN + 11));
    assert_eq!((e_single, e_double), (1.5, -2.25));
}

#[test]
fn every_enum_shape_round_trips() {
    let (provider, lab) = setup();
    let to = Address::new_from_array([9; 32]);

    send!(&provider, lab, build_set_action, SetActionAccounts, Action::Transfer { amount: 777, to })
        .expect("set_action Transfer");
    match read(&provider, &lab).action.get() {
        Ok(Action::Transfer { amount, to: got }) => {
            assert_eq!(amount, 777);
            assert_eq!(got, to);
        }
        _ => panic!("expected the Transfer variant"),
    }
    send!(&provider, lab, build_set_action, SetActionAccounts, Action::Paint(Color::Blue))
        .expect("set_action Paint");
    assert!(matches!(
        read(&provider, &lab).action.get(),
        Ok(Action::Paint(Color::Blue))
    ));
    send!(&provider, lab, build_set_action, SetActionAccounts, Action::Pause).expect("Pause");
    assert!(matches!(read(&provider, &lab).action.get(), Ok(Action::Pause)));

    send!(&provider, lab, build_set_color, SetColorAccounts, Color::Blue).expect("set_color");
    assert!(matches!(read(&provider, &lab).color.get(), Ok(Color::Blue)));
    send!(&provider, lab, build_set_level, SetLevelAccounts, Level::High).expect("set_level");
    assert!(matches!(read(&provider, &lab).level.get(), Ok(Level::High)));
    send!(&provider, lab, build_set_signed, SetSignedAccounts, Signed::Pos).expect("set_signed");
    assert!(matches!(read(&provider, &lab).signed.get(), Ok(Signed::Pos)));
    send!(&provider, lab, build_set_wide, SetWideAccounts, Wide::Big { value: 9_000_000_000 })
        .expect("set_wide");
    assert!(matches!(
        read(&provider, &lab).wide.get(),
        Ok(Wide::Big { value: 9_000_000_000 })
    ));
}

#[test]
fn nested_structs_arrays_and_options_round_trip() {
    let (provider, lab) = setup();
    let args = NestedArgs {
        limits: Limits {
            min: 5,
            max: 50,
            mode: Color::Blue.into(),
        },
        matrix: [1, 2, 3, 4],
        bytes: [9; 8],
    };
    send!(&provider, lab, build_set_nested, SetNestedAccounts, args).expect("set_nested");
    let state = read(&provider, &lab);
    let limits = state.limits;
    let (min, max) = (limits.min, limits.max);
    assert_eq!((min, max), (5, 50));
    assert!(matches!(limits.mode.get(), Ok(Color::Blue)));
    let (matrix, bytes) = (state.matrix, state.bytes);
    assert_eq!((matrix, bytes), ([1, 2, 3, 4], [9; 8]));

    send!(&provider, lab, build_set_maybe_u64, SetMaybeU64Accounts, OptionArgs { maybe: Some(77) })
        .expect("set_maybe_u64 Some");
    assert_eq!(read(&provider, &lab).maybe_u64.get(), Some(77));
    send!(&provider, lab, build_set_maybe_u64, SetMaybeU64Accounts, OptionArgs { maybe: None })
        .expect("set_maybe_u64 None");
    assert_eq!(read(&provider, &lab).maybe_u64.get(), None);

    send!(&provider, lab, build_set_maybe_color, SetMaybeColorAccounts, Color::Blue)
        .expect("set_maybe_color");
    let maybe = read(&provider, &lab).maybe_color.get();
    assert!(matches!(maybe.map(|c| c.get()), Some(Ok(Color::Blue))));
    send!(&provider, lab, build_clear_maybe_color, ClearMaybeColorAccounts).expect("clear");
    assert!(read(&provider, &lab).maybe_color.get().is_none());
}

#[test]
fn dynamic_arguments_fill_the_fixed_capacity_fields() {
    let (provider, lab) = setup();
    send!(&provider, lab, build_set_text, SetTextAccounts, "naclac".to_string()).expect("text");
    assert_eq!(read(&provider, &lab).name.as_str().ok(), Some("naclac"));

    send!(&provider, lab, build_set_scores, SetScoresAccounts, vec![10u64, 20, 30]).expect("scores");
    let state = read(&provider, &lab);
    assert_eq!(state.scores.len(), 3);
    assert_eq!(state.scores.get(1), Some(20));

    let mut scores = FixedVec::<u64, 4>::new();
    assert!(scores.try_push(7).is_ok());
    let mut name = FixedString::<16>::new();
    assert!(name.try_set("roster").is_ok());
    send!(&provider, lab, build_set_roster, SetRosterAccounts, scores, name).expect("roster");
    let state = read(&provider, &lab);
    assert_eq!(state.scores.len(), 1);
    assert_eq!(state.name.as_str().ok(), Some("roster"));

    send!(&provider, lab, build_clear_roster, ClearRosterAccounts).expect("clear_roster");
    assert!(read(&provider, &lab).scores.is_empty());
}

#[test]
fn capacity_limits_and_program_errors_surface_with_their_codes() {
    let (provider, lab) = setup();
    for value in 1..=4u64 {
        send!(&provider, lab, build_push_score, PushScoreAccounts, value).expect("push_score");
    }
    assert_custom(
        send!(&provider, lab, build_push_score, PushScoreAccounts, 5u64),
        ROSTER_FULL,
    );
    for index in 0..3u8 {
        let member = Address::new_from_array([index + 1; 32]);
        send!(&provider, lab, build_add_member, AddMemberAccounts, member).expect("add_member");
    }
    assert_custom(
        send!(&provider, lab, build_add_member, AddMemberAccounts, Address::new_from_array([9; 32])),
        CAPACITY_EXCEEDED,
    );
    assert_custom(
        send!(&provider, lab, build_set_scores, SetScoresAccounts, vec![1u64, 2, 3, 4, 5]),
        CAPACITY_EXCEEDED,
    );
    for flag in 0..8u8 {
        send!(&provider, lab, build_push_flag, PushFlagAccounts, flag % 2).expect("push_flag");
    }
    assert_custom(
        send!(&provider, lab, build_push_flag, PushFlagAccounts, 1u8),
        CAPACITY_EXCEEDED,
    );
    let state = read(&provider, &lab);
    let flags: Vec<u8> = state.flags.iter().map(|flag| flag.0).collect();
    assert_eq!(flags, vec![0, 1, 0, 1, 0, 1, 0, 1]);

    send!(&provider, lab, build_remove_member, RemoveMemberAccounts, 0u8).expect("remove_member");
    assert_eq!(read(&provider, &lab).members.len(), 2);
    assert_custom(
        send!(&provider, lab, build_remove_member, RemoveMemberAccounts, 9u8),
        INVALID_INSTRUCTION_DATA,
    );

    assert_custom(send!(&provider, lab, build_fail_with, FailWithAccounts, 1u8), REJECTED);
    send!(&provider, lab, build_fail_with, FailWithAccounts, 0u8).expect("fail_with 0 succeeds");
}

#[test]
fn popping_an_empty_roster_and_an_overflowing_counter_are_rejected() {
    let (provider, lab) = setup();
    assert_custom(
        send!(&provider, lab, build_pop_score, PopScoreAccounts),
        INVALID_INSTRUCTION_DATA,
    );

    send!(&provider, lab, build_bump_counter, BumpCounterAccounts).expect("bump_counter");
    let counter_at = LAB_DISCRIMINATOR.len() + core::mem::offset_of!(Lab, counter);
    let mut data = provider.get_account_data(&lab).unwrap();
    let owner = provider.get_account(&lab).unwrap().owner;
    let lamports = provider.get_balance(&lab).unwrap();
    data[counter_at] = u8::MAX;
    provider.set_account(&lab, data, &owner, lamports).unwrap();
    assert_custom(send!(&provider, lab, build_bump_counter, BumpCounterAccounts), OVERFLOWED);
}

#[test]
fn emit_log_writes_fixed_and_alloc_events() {
    let (provider, lab) = setup();
    send!(&provider, lab, build_set_text, SetTextAccounts, "events".to_string()).expect("text");
    send!(&provider, lab, build_set_scores, SetScoresAccounts, vec![1u64, 2]).expect("scores");
    send!(&provider, lab, build_set_maybe_color, SetMaybeColorAccounts, Color::Blue).expect("color");
    let meta = send!(&provider, lab, build_emit_log, EmitLogAccounts).expect("emit_log");

    let enums = meta.parse_events_zero_copy::<EnumsChanged>().expect("enums event");
    assert_eq!(enums.len(), 1);
    assert!(matches!(enums[0].color.get(), Ok(Color::Green)));
    assert!(matches!(enums[0].signed.get(), Ok(Signed::Neg)));

    let rosters = meta.parse_events_zero_copy::<RosterChanged>().expect("roster event");
    assert_eq!(rosters.len(), 1);
    assert_eq!(rosters[0].scores.len(), 2);
    assert_eq!(rosters[0].name.as_str().ok(), Some("events"));

    let logs = meta.parse_events_alloc::<LabLog>().expect("lab log");
    assert_eq!(logs.len(), 1);
    assert_eq!(logs[0].note, "lab");
    assert_eq!(logs[0].values, vec![1u64, 2, 3]);
    assert_eq!(logs[0].colors.len(), 2);
    assert!(matches!(logs[0].maybe, Some(Color::Blue)));

    let texts = meta.parse_events_alloc::<TextLog>().expect("text log");
    assert_eq!(texts.len(), 1);
    assert_eq!(texts[0].note, "text");
    assert_eq!(texts[0].name.as_str().ok(), Some("events"));
}

#[test]
fn emit_cpi_records_both_event_kinds_in_the_inner_instruction_trace() {
    let (provider, lab) = setup();
    let event_authority = Address::find_program_address(&[b"__event_authority"], &PROGRAM_ID).0;
    fresh_blockhash(&provider);
    let meta = build_emit_cpi_log(
        &provider,
        PROGRAM_ID,
        EmitCpiLogAccounts {
            signer: provider.payer.address(),
            lab,
            event_authority,
            program: PROGRAM_ID,
        },
    )
    .send_and_confirm()
    .expect("emit_cpi_log should succeed");

    let inner = meta
        .inner_instructions
        .as_ref()
        .expect("litesvm must supply inner instructions");
    assert_eq!(inner.len(), 2, "two emit_cpi! calls are two inner instructions");
    for call in inner {
        assert_eq!(call.outer_index, 0);
        assert_eq!(call.stack_height, 2);
        assert_eq!(call.program_id, PROGRAM_ID);
        assert_eq!(call.accounts, vec![event_authority]);
    }
    assert_eq!(
        &inner[0].data[2..2 + EnumsChanged::DISCRIMINATOR.len()],
        &EnumsChanged::DISCRIMINATOR[..]
    );
    assert_eq!(
        &inner[1].data[2..2 + LabLog::DISCRIMINATOR.len()],
        &LabLog::DISCRIMINATOR[..]
    );

    assert_eq!(meta.parse_events_zero_copy::<EnumsChanged>().expect("enums").len(), 1);
    let logs = meta.parse_events_alloc::<LabLog>().expect("lab log");
    assert_eq!(logs.len(), 1);
    assert_eq!(logs[0].note, "cpi");
    assert_eq!(logs[0].values, vec![4u64, 5]);
}

#[test]
fn an_optional_account_is_used_when_present_and_skipped_when_absent() {
    let (provider, _lab) = setup();
    let (note, _bump) = get_note_pda(&PROGRAM_ID);
    build_init_note(
        &provider,
        PROGRAM_ID,
        InitNoteAccounts {
            payer: provider.payer.address(),
            note,
            system_program: Address::default(),
        },
    )
    .send_and_confirm()
    .expect("init_note should succeed");

    fresh_blockhash(&provider);
    build_touch_optional(
        &provider,
        PROGRAM_ID,
        TouchOptionalAccounts {
            signer: provider.payer.address(),
            note: Some(note),
        },
    )
    .send_and_confirm()
    .expect("touch_optional with the note should succeed");
    let value = fetch_note(&provider, &note).expect("note decodes").value;
    assert_eq!(value, 1);

    fresh_blockhash(&provider);
    build_touch_optional(
        &provider,
        PROGRAM_ID,
        TouchOptionalAccounts {
            signer: provider.payer.address(),
            note: None,
        },
    )
    .send_and_confirm()
    .expect("touch_optional without the note should succeed");
    let value = fetch_note(&provider, &note).expect("note decodes").value;
    assert_eq!(value, 1);
}

#[test]
fn a_defined_type_with_fixed_capacity_fields_is_stored_in_an_array_account() {
    let (provider, _lab) = setup();
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

    let mut scores = FixedVec::<u64, 4>::new();
    assert!(scores.try_push(3).is_ok());
    assert!(scores.try_push(4).is_ok());
    let mut name = FixedString::<8>::new();
    assert!(name.try_set("entry").is_ok());
    fresh_blockhash(&provider);
    build_set_entry(
        &provider,
        PROGRAM_ID,
        Entry { scores, name },
        SetEntryAccounts {
            signer: provider.payer.address(),
            book,
        },
    )
    .send_and_confirm()
    .expect("set_entry should succeed");

    let first = fetch_book(&provider, &book).expect("book decodes").first;
    assert_eq!(first.scores.len(), 2);
    assert_eq!(first.scores.get(1), Some(4));
    assert_eq!(first.name.as_str().ok(), Some("entry"));
}

#[test]
fn discriminators_have_the_custom_widths() {
    let (provider, lab) = setup();
    assert_eq!(&LAB_DISCRIMINATOR[..], &[3u8, 0, 0, 1][..]);
    assert_eq!(&ScalarsSet::DISCRIMINATOR[..], &[2u8, 0, 1][..]);
    assert_eq!(&LabLog::DISCRIMINATOR[..], &[2u8, 0, 4][..]);
    let data = build_set_color(
        &provider,
        PROGRAM_ID,
        Color::Blue,
        SetColorAccounts {
            signer: provider.payer.address(),
            lab,
        },
    )
    .build_instruction()
    .data;
    assert_eq!(&data[..2], &[1u8, 3][..]);
    assert_eq!(data.len(), 3);
}
