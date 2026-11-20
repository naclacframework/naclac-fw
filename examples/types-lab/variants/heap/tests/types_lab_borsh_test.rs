//! End-to-end LiteSVM tests of the heap family (borsh and wincode): every
//! instruction goes through the generated Rust client to the compiled
//! program, accounts are read back and events are decoded from the
//! transaction metadata.

use naclac_client::*;
use types_lab_borsh_client::{
    fetch_lab, fetch_note, get_lab_pda, get_note_pda,
    instructions::*,
    types::{
        Action, Color, EnumsChanged, LabLog, Level, Limits, NestedArgs, OptionArgs,
        RosterChanged, ScalarArgs, ScalarsSet, Wide, PROGRAM_ID,
    },
    Lab, LAB_DISCRIMINATOR,
};

const INVALID_INSTRUCTION_DATA: u32 = 10_012;
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

macro_rules! events_of {
    ($meta:expr, $ty:ty) => {{
        #[cfg(feature = "borsh")]
        let found: Vec<$ty> = $meta.parse_events_borsh().expect("events should parse");
        #[cfg(feature = "wincode")]
        let found: Vec<$ty> = $meta.parse_events_wincode().expect("events should parse");
        found
    }};
}

fn read(provider: &NaclacProvider, lab: &Address) -> Lab {
    fetch_lab(provider, lab).expect("lab should decode")
}

fn scalar_args() -> ScalarArgs {
    ScalarArgs {
        flag: true,
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
fn init_writes_the_chosen_variants_and_defaults_elsewhere() {
    let (provider, lab) = setup();
    let state = read(&provider, &lab);
    assert_eq!(state.authority, provider.payer.address());
    assert!(matches!(state.color, Color::Green));
    assert!(matches!(state.action, Action::SetFee { fee_bps: 25 }));
    assert!(matches!(state.level, Level::Mid));
    assert!(matches!(state.wide, Wide::Small(5)));
    assert!(matches!(state.history[0], Color::Blue));
    assert!(matches!(state.history[1], Color::Red));
    assert!(state.maybe_u64.is_none());
    assert!(state.scores.is_empty() && state.members.is_empty() && state.flags.is_empty());
    assert!(state.name.is_empty());
    assert_eq!(state.counter, 0);
}

#[test]
fn every_scalar_type_round_trips_through_the_account_and_the_event() {
    let (provider, lab) = setup();
    let meta = send!(&provider, lab, build_set_scalars, SetScalarsAccounts, scalar_args())
        .expect("set_scalars should succeed");

    let state = read(&provider, &lab);
    assert!(state.flag);
    assert_eq!((state.small, state.short, state.medium), (200, 60_000, 4_000_000_000));
    assert_eq!((state.large, state.huge), (u64::MAX - 5, u128::MAX - 7));
    assert_eq!(
        (state.tiny_signed, state.short_signed, state.medium_signed),
        (-100, -30_000, -2_000_000_000)
    );
    assert_eq!((state.large_signed, state.huge_signed), (i64::MIN + 3, i128::MIN + 11));
    assert_eq!((state.single, state.double), (1.5, -2.25));
    assert_eq!(state.key, Address::new_from_array([4; 32]));

    let events = events_of!(meta, ScalarsSet);
    assert_eq!(events.len(), 1);
    assert!(events[0].flag);
    assert_eq!((events[0].small, events[0].huge), (200, u128::MAX - 7));
    assert_eq!(events[0].huge_signed, i128::MIN + 11);
    assert_eq!((events[0].single, events[0].double), (1.5, -2.25));
}

#[test]
fn every_enum_shape_round_trips() {
    let (provider, lab) = setup();
    let to = Address::new_from_array([9; 32]);

    send!(&provider, lab, build_set_action, SetActionAccounts, Action::Transfer { amount: 777, to })
        .expect("set_action Transfer");
    match read(&provider, &lab).action {
        Action::Transfer { amount, to: got } => {
            assert_eq!(amount, 777);
            assert_eq!(got, to);
        }
        _ => panic!("expected the Transfer variant"),
    }
    send!(&provider, lab, build_set_action, SetActionAccounts, Action::Paint(Color::Blue))
        .expect("set_action Paint");
    assert!(matches!(read(&provider, &lab).action, Action::Paint(Color::Blue)));
    send!(&provider, lab, build_set_action, SetActionAccounts, Action::Pause).expect("Pause");
    assert!(matches!(read(&provider, &lab).action, Action::Pause));

    send!(&provider, lab, build_set_color, SetColorAccounts, Color::Blue).expect("set_color");
    assert!(matches!(read(&provider, &lab).color, Color::Blue));
    send!(&provider, lab, build_set_level, SetLevelAccounts, Level::High).expect("set_level");
    assert!(matches!(read(&provider, &lab).level, Level::High));
    send!(&provider, lab, build_set_wide, SetWideAccounts, Wide::Big { value: 9_000_000_000 })
        .expect("set_wide");
    assert!(matches!(
        read(&provider, &lab).wide,
        Wide::Big { value: 9_000_000_000 }
    ));
}

#[test]
fn nested_structs_arrays_and_options_round_trip() {
    let (provider, lab) = setup();
    let args = NestedArgs {
        limits: Limits {
            min: 5,
            max: 50,
            mode: Color::Blue,
        },
        matrix: [1, 2, 3, 4],
        bytes: [9; 8],
    };
    send!(&provider, lab, build_set_nested, SetNestedAccounts, args).expect("set_nested");
    let state = read(&provider, &lab);
    assert_eq!((state.limits.min, state.limits.max), (5, 50));
    assert!(matches!(state.limits.mode, Color::Blue));
    assert_eq!((state.matrix, state.bytes), ([1, 2, 3, 4], [9; 8]));

    send!(&provider, lab, build_set_maybe_u64, SetMaybeU64Accounts, OptionArgs { maybe: Some(77) })
        .expect("set_maybe_u64 Some");
    assert_eq!(read(&provider, &lab).maybe_u64, Some(77));
    send!(&provider, lab, build_set_maybe_u64, SetMaybeU64Accounts, OptionArgs { maybe: None })
        .expect("set_maybe_u64 None");
    assert_eq!(read(&provider, &lab).maybe_u64, None);

    send!(&provider, lab, build_set_maybe_color, SetMaybeColorAccounts, Color::Blue)
        .expect("set_maybe_color");
    assert!(matches!(read(&provider, &lab).maybe_color, Some(Color::Blue)));
    send!(&provider, lab, build_clear_maybe_color, ClearMaybeColorAccounts).expect("clear");
    assert!(read(&provider, &lab).maybe_color.is_none());
}

#[test]
fn dynamic_arguments_fill_the_sized_fields() {
    let (provider, lab) = setup();
    send!(&provider, lab, build_set_text, SetTextAccounts, "naclac".to_string()).expect("text");
    assert_eq!(read(&provider, &lab).name, "naclac");

    send!(&provider, lab, build_set_scores, SetScoresAccounts, vec![10u64, 20, 30]).expect("scores");
    assert_eq!(read(&provider, &lab).scores, vec![10u64, 20, 30]);

    send!(
        &provider,
        lab,
        build_set_roster,
        SetRosterAccounts,
        vec![7u64],
        "roster".to_string()
    )
    .expect("roster");
    let state = read(&provider, &lab);
    assert_eq!(state.scores, vec![7u64]);
    assert_eq!(state.name, "roster");

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
        ROSTER_FULL,
    );
    assert_custom(
        send!(&provider, lab, build_set_scores, SetScoresAccounts, vec![1u64, 2, 3, 4, 5]),
        ROSTER_FULL,
    );
    assert_custom(
        send!(&provider, lab, build_set_text, SetTextAccounts, "x".repeat(17)),
        REJECTED,
    );
    for flag in 0..8u8 {
        send!(&provider, lab, build_push_flag, PushFlagAccounts, flag % 2).expect("push_flag");
    }
    assert_custom(
        send!(&provider, lab, build_push_flag, PushFlagAccounts, 1u8),
        ROSTER_FULL,
    );
    assert_eq!(
        read(&provider, &lab).flags,
        vec![false, true, false, true, false, true, false, true]
    );

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
    for _ in 0..u8::MAX {
        send!(&provider, lab, build_bump_counter, BumpCounterAccounts).expect("bump_counter");
    }
    assert_eq!(read(&provider, &lab).counter, u8::MAX);
    assert_custom(send!(&provider, lab, build_bump_counter, BumpCounterAccounts), OVERFLOWED);
}

#[test]
fn emit_log_writes_events_with_dynamic_fields() {
    let (provider, lab) = setup();
    send!(&provider, lab, build_set_text, SetTextAccounts, "events".to_string()).expect("text");
    send!(&provider, lab, build_set_scores, SetScoresAccounts, vec![1u64, 2]).expect("scores");
    send!(&provider, lab, build_set_maybe_color, SetMaybeColorAccounts, Color::Blue).expect("color");
    let meta = send!(&provider, lab, build_emit_log, EmitLogAccounts).expect("emit_log");

    let enums = events_of!(meta, EnumsChanged);
    assert_eq!(enums.len(), 1);
    assert!(matches!(enums[0].color, Color::Green));
    assert!(matches!(enums[0].wide, Wide::Small(5)));

    let rosters = events_of!(meta, RosterChanged);
    assert_eq!(rosters.len(), 1);
    assert_eq!(rosters[0].scores, vec![1u64, 2]);
    assert_eq!(rosters[0].name, "events");

    let logs = events_of!(meta, LabLog);
    assert_eq!(logs.len(), 1);
    assert_eq!(logs[0].note, "lab");
    assert_eq!(logs[0].values, vec![1u64, 2, 3]);
    assert_eq!(logs[0].colors.len(), 2);
    assert!(matches!(logs[0].maybe, Some(Color::Blue)));
}

#[test]
fn emit_cpi_records_both_events_in_the_inner_instruction_trace() {
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

    assert_eq!(events_of!(meta, EnumsChanged).len(), 1);
    let logs = events_of!(meta, LabLog);
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
    assert_eq!(fetch_note(&provider, &note).expect("note decodes").value, 1);

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
    assert_eq!(fetch_note(&provider, &note).expect("note decodes").value, 1);
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
