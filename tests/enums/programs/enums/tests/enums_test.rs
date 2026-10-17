//! End-to-end LiteSVM tests for `ZcEnum` fields: every write and read goes
//! through the generated Rust client against the compiled program, and the
//! compute units each operation costs are printed from the real execution
//! metadata (run with `--nocapture`).

use enums_client::{
    get_state_pda,
    instructions::*,
    types::{
        Action, Color, DynArgs, ListArgs, ModeArgs, MixedEvent, StateChanged, StateLog,
        PROGRAM_ID,
    },
    State, STATE_DISCRIMINATOR,
};
use naclac_client::*;

fn setup() -> (NaclacProvider, Address) {
    let payer = load_node_wallet().expect("Failed to load local Solana keypair");
    let provider = NaclacProvider::new("litesvm", payer).expect("Failed to construct provider");
    let (state, _bump) = get_state_pda(&PROGRAM_ID);
    build_init_state(
        &provider,
        PROGRAM_ID,
        InitStateAccounts {
            payer: provider.payer.address(),
            state,
            system_program: Address::default(),
        },
    )
    .send_and_confirm()
    .expect("init_state should succeed");
    (provider, state)
}

fn read_state(provider: &NaclacProvider, state: &Address) -> State {
    enums_client::fetch_state(provider, state).expect("state should decode")
}

fn mutate_accounts(provider: &NaclacProvider, state: Address) -> (Address, Address) {
    (provider.payer.address(), state)
}

fn set_action(provider: &NaclacProvider, state: Address, action: Action) {
    let (signer, state) = mutate_accounts(provider, state);
    build_set_action(
        provider,
        PROGRAM_ID,
        action,
        SetActionAccounts { signer, state },
    )
    .send_and_confirm()
    .expect("set_action should succeed");
}

fn set_color(provider: &NaclacProvider, state: Address, color: Color) {
    let (signer, state) = mutate_accounts(provider, state);
    build_set_color(
        provider,
        PROGRAM_ID,
        color,
        SetColorAccounts { signer, state },
    )
    .send_and_confirm()
    .expect("set_color should succeed");
}

#[test]
fn init_writes_the_chosen_variants_and_zero_tags_elsewhere() {
    let (provider, state) = setup();
    let s = read_state(&provider, &state);

    assert!(matches!(s.color.get(), Ok(Color::Green)));
    assert!(matches!(s.action.get(), Ok(Action::SetFee { fee_bps: 25 })));
    for slot in s.history {
        assert!(matches!(slot.get(), Ok(Color::Red)));
    }
    assert_eq!(s.maybe_color.tag, 0);
    assert!(matches!(s.limits.mode.get(), Ok(Color::Red)));
    assert_eq!(s.counter, 0);
}

#[test]
fn set_action_round_trips_every_variant_shape() {
    let (provider, state) = setup();
    let to = Address::new_from_array([9; 32]);

    set_action(&provider, state, Action::Transfer { amount: 777, to });
    match read_state(&provider, &state).action.get() {
        Ok(Action::Transfer { amount, to: got }) => {
            assert_eq!(amount, 777);
            assert_eq!(got, to);
        }
        _ => panic!("expected the Transfer variant"),
    }

    set_action(&provider, state, Action::Paint(Color::Blue));
    assert!(matches!(
        read_state(&provider, &state).action.get(),
        Ok(Action::Paint(Color::Blue))
    ));

    set_action(&provider, state, Action::Pause);
    assert!(matches!(
        read_state(&provider, &state).action.get(),
        Ok(Action::Pause)
    ));
}

#[test]
fn set_color_round_trips_a_non_contiguous_discriminant() {
    let (provider, state) = setup();
    set_color(&provider, state, Color::Blue);
    assert!(matches!(
        read_state(&provider, &state).color.get(),
        Ok(Color::Blue)
    ));
}

#[test]
fn an_invalid_enum_byte_in_the_account_is_rejected_on_chain() {
    let (provider, state) = setup();
    let mut data = provider.get_account_data(&state).unwrap();
    let owner = provider.get_account(&state).unwrap().owner;
    let lamports = provider.get_balance(&state).unwrap();

    let color_offset = STATE_DISCRIMINATOR.len() + core::mem::offset_of!(State, color);
    data[color_offset] = 5;
    provider.set_account(&state, data, &owner, lamports).unwrap();

    let result = build_bench_color_get(
        &provider,
        PROGRAM_ID,
        BenchColorGetAccounts {
            signer: provider.payer.address(),
            state,
        },
    )
    .send_and_confirm();
    assert!(
        matches!(
            result,
            Err(NaclacClientError::TransactionFailed {
                instruction_err: InstructionError::InvalidAccountData,
                ..
            })
        ),
        "an enum byte with no variant must fail with InvalidAccountData, got {result:?}"
    );
}

#[test]
fn exact_compute_units_per_operation() {
    let (provider, state) = setup();
    let meta = build_measure_cu(
        &provider,
        PROGRAM_ID,
        MeasureCuAccounts {
            signer: provider.payer.address(),
            state,
        },
    )
    .send_and_confirm()
    .expect("measure_cu should succeed");

    let remaining: Vec<u64> = meta
        .logs
        .iter()
        .filter_map(|line| {
            line.strip_prefix("Program consumption: ")?
                .strip_suffix(" units remaining")?
                .parse()
                .ok()
        })
        .collect();
    assert_eq!(
        remaining.len(),
        12,
        "expected one pair of logs for the empty bracket and one per operation"
    );

    // Each pair logs the remaining units before and after; the second log's own
    // cost is inside the difference, and the empty pair measures exactly that.
    let empty = remaining[0] - remaining[1];
    let cost = |pair: usize| (remaining[2 * pair] - remaining[2 * pair + 1]).saturating_sub(empty);

    println!("\nexact compute units, measured inside the program (logging overhead subtracted)");
    println!("{:<32} {:>8}", "operation", "CU");
    println!("{:<32} {:>8}", "empty pair (subtracted)", empty);
    println!("{:<32} {:>8}", "u8 field = const", cost(1));
    println!("{:<32} {:>8}", "ZcEnum<Color>.get() + match", cost(2));
    println!("{:<32} {:>8}", "ZcEnum<Action>.get() + match", cost(3));
    println!("{:<32} {:>8}", "ZcEnum<Color>.set(const)", cost(4));
    println!("{:<32} {:>8}", "ZcEnum<Action>.set(Transfer)", cost(5));
}

#[test]
fn an_enum_in_instruction_args_reaches_the_program_intact() {
    let (provider, state) = setup();
    let args = ModeArgs {
        mode: Color::Blue.into(),
        amount: 9,
    };
    build_set_mode_with_args(
        &provider,
        PROGRAM_ID,
        args,
        SetModeWithArgsAccounts {
            signer: provider.payer.address(),
            state,
        },
    )
    .send_and_confirm()
    .expect("set_mode_with_args should succeed");

    let s = read_state(&provider, &state);
    assert!(matches!(s.color.get(), Ok(Color::Blue)));
    assert_eq!(s.counter, 9);
}

#[test]
fn an_event_carrying_enums_decodes_to_the_emitted_values() {
    let (provider, state) = setup();
    set_action(
        &provider,
        state,
        Action::Transfer {
            amount: 42,
            to: Address::new_from_array([3; 32]),
        },
    );
    set_color(&provider, state, Color::Blue);

    let meta = build_emit_state_changed(
        &provider,
        PROGRAM_ID,
        EmitStateChangedAccounts {
            signer: provider.payer.address(),
            state,
        },
    )
    .send_and_confirm()
    .expect("emit_state_changed should succeed");

    let events = meta
        .parse_events_zero_copy::<StateChanged>()
        .expect("events should parse");
    assert_eq!(events.len(), 1, "exactly one StateChanged event expected");
    let event = events[0];
    assert!(matches!(event.color.get(), Ok(Color::Blue)));
    match event.action.get() {
        Ok(Action::Transfer { amount, to }) => {
            assert_eq!(amount, 42);
            assert_eq!(to, Address::new_from_array([3; 32]));
        }
        _ => panic!("expected the Transfer variant"),
    }
}

#[test]
fn compute_units_of_reading_each_action_variant() {
    let variants = [
        ("Pause", Action::Pause),
        ("SetFee", Action::SetFee { fee_bps: 25 }),
        (
            "Transfer",
            Action::Transfer {
                amount: 5,
                to: Address::new_from_array([1; 32]),
            },
        ),
        ("Paint(Blue)", Action::Paint(Color::Blue)),
    ];

    println!("\n{:<16} {:>8}", "Action variant", "get CU");
    for (name, action) in variants {
        let (provider, state) = setup();
        set_action(&provider, state, action);
        let cu = build_bench_action_get(
            &provider,
            PROGRAM_ID,
            BenchActionGetAccounts {
                signer: provider.payer.address(),
                state,
            },
        )
        .send_and_confirm()
        .expect("bench_action_get should succeed")
        .compute_units_consumed;
        println!("{:<16} {:>8}", name, cu);
    }
}

#[test]
fn the_account_bytes_match_the_golden_fixture_shared_with_the_typescript_client() {
    let (provider, state) = setup();
    set_action(
        &provider,
        state,
        Action::Transfer {
            amount: 777,
            to: Address::new_from_array([9; 32]),
        },
    );
    set_color(&provider, state, Color::Blue);

    let mut data = provider.get_account_data(&state).unwrap();
    let admin_at = STATE_DISCRIMINATOR.len() + core::mem::offset_of!(State, admin);
    data[admin_at..admin_at + 32].fill(0);
    data[STATE_DISCRIMINATOR.len() + core::mem::offset_of!(State, bump)] = 0;
    let hex: String = data.iter().map(|b| format!("{b:02x}")).collect();

    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../golden/state_zc.hex");
    if std::env::var_os("UPDATE_GOLDEN").is_some() {
        std::fs::write(&path, &hex).unwrap();
    }
    let expected = std::fs::read_to_string(&path).expect("golden fixture must exist");
    assert_eq!(hex, expected.trim());
}

#[test]
fn an_alloc_event_carrying_enums_decodes_to_the_emitted_values() {
    let (provider, state) = setup();
    set_action(
        &provider,
        state,
        Action::Transfer {
            amount: 42,
            to: Address::new_from_array([3; 32]),
        },
    );
    set_color(&provider, state, Color::Blue);

    let meta = build_emit_state_log(
        &provider,
        PROGRAM_ID,
        EmitStateLogAccounts {
            signer: provider.payer.address(),
            state,
        },
    )
    .send_and_confirm()
    .expect("emit_state_log should succeed");

    let events = meta
        .parse_events_alloc::<StateLog>()
        .expect("events should parse");
    assert_eq!(events.len(), 1, "exactly one StateLog event expected");
    let event = &events[0];
    assert_eq!(event.head, 7);
    assert!(matches!(event.color, Color::Blue));
    assert_eq!(event.colors.len(), 2);
    assert!(matches!(event.colors[0], Color::Red));
    assert!(matches!(event.colors[1], Color::Blue));
    assert!(matches!(event.maybe, Some(Color::Blue)));
    assert!(matches!(
        event.action,
        Action::Transfer { amount: 42, .. }
    ));
}

fn assert_matches_zero_copy_golden(instruction_data: &[u8], name: &str) {
    let hex: String = instruction_data.iter().map(|b| format!("{b:02x}")).collect();
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../golden")
        .join(name);
    let expected = std::fs::read_to_string(&path)
        .expect("golden fixture must exist (written by the enums_zc tests)");
    assert_eq!(hex, expected.trim(), "Pinocchio must encode exactly like zero-copy");
}

fn send_with_data(
    provider: &NaclacProvider,
    state: Address,
    data: Vec<u8>,
) -> Result<NaclacTransactionMetadata, NaclacClientError> {
    naclac_client::InstructionBuilder::new(provider, PROGRAM_ID, data)
        .account(
            naclac_client::AccountMeta::new_readonly(provider.payer.address(), true),
            "signer",
        )
        .account(naclac_client::AccountMeta::new(state, false), "state")
        .send_and_confirm()
}

fn dyn_args_instruction(
    provider: &NaclacProvider,
    state: Address,
) -> naclac_client::InstructionBuilder<'_> {
    build_dyn_args(
        provider,
        PROGRAM_ID,
        DynArgs {
            tag: 3,
            values: vec![10, 20, 30, 40],
            note: "hello".to_string(),
            maybe: Some(500),
            heap: vec![1, 2],
        },
        vec![7, 8, 9],
        DynArgsAccounts {
            signer: provider.payer.address(),
            state,
        },
    )
}

fn text_lists_instruction(
    provider: &NaclacProvider,
    state: Address,
) -> naclac_client::InstructionBuilder<'_> {
    build_text_lists(
        provider,
        PROGRAM_ID,
        ListArgs {
            words: vec!["ab".to_string(), "cde".to_string()],
        },
        vec!["x".to_string(), "yz".to_string()],
        vec![vec![1u8, 2, 3, 4]],
        TextListsAccounts {
            signer: provider.payer.address(),
            state,
        },
    )
}

fn enum_lists_instruction(
    provider: &NaclacProvider,
    state: Address,
) -> naclac_client::InstructionBuilder<'_> {
    build_enum_lists(
        provider,
        PROGRAM_ID,
        vec![Color::Blue, Color::Red, Color::Blue],
        vec![Color::Blue, Color::Green],
        EnumListsAccounts {
            signer: provider.payer.address(),
            state,
        },
    )
}

#[test]
fn dynamic_instruction_args_reach_the_program_intact() {
    let (provider, state) = setup();
    dyn_args_instruction(&provider, state)
        .send_and_confirm()
        .expect("dyn_args should succeed");
    assert_eq!(read_state(&provider, &state).counter, 117);
}

#[test]
fn lists_of_text_and_bytes_reach_the_program_intact() {
    let (provider, state) = setup();
    text_lists_instruction(&provider, state)
        .send_and_confirm()
        .expect("text_lists should succeed");
    assert_eq!(read_state(&provider, &state).counter, 12);
}

#[test]
fn vectors_of_enums_reach_the_program_intact() {
    let (provider, state) = setup();
    enum_lists_instruction(&provider, state)
        .send_and_confirm()
        .expect("enum_lists should succeed");
    assert_eq!(read_state(&provider, &state).counter, 21);
}

#[test]
fn invalid_utf8_in_a_list_item_is_rejected_not_a_panic() {
    let (provider, state) = setup();
    let mut data = text_lists_instruction(&provider, state).build_instruction().data;
    data[16] = 0xFF;
    let result = send_with_data(&provider, state, data);
    assert!(
        matches!(
            result,
            Err(NaclacClientError::TransactionFailed {
                instruction_err: InstructionError::Custom(10012),
                ..
            })
        ),
        "invalid UTF-8 must be InvalidInstructionData, got {result:?}"
    );
}

#[test]
fn an_invalid_enum_tag_in_a_vector_argument_is_rejected_not_a_panic() {
    let (provider, state) = setup();
    let template = enum_lists_instruction(&provider, state).build_instruction();

    let mut heap_data = template.data.clone();
    heap_data[8 + 4] = 99;
    let heap_result = send_with_data(&provider, state, heap_data);
    assert!(
        matches!(
            heap_result,
            Err(NaclacClientError::TransactionFailed {
                instruction_err: InstructionError::Custom(10012),
                ..
            })
        ),
        "a bad tag in the heap Vec must be InvalidInstructionData, got {heap_result:?}"
    );

    let mut view_data = template.data.clone();
    view_data[8 + 4 + 3 + 4] = 99;
    let view_result = send_with_data(&provider, state, view_data);
    assert!(
        matches!(
            view_result,
            Err(NaclacClientError::TransactionFailed {
                instruction_err: InstructionError::InvalidAccountData,
                ..
            })
        ),
        "a bad tag in the ZcVec must fail the element read, got {view_result:?}"
    );
}

#[test]
fn a_fixed_event_of_mixed_field_sizes_is_packed_on_the_wire_and_decodes() {
    let (provider, state) = setup();
    set_color(&provider, state, Color::Blue);

    let meta = build_emit_mixed_event(
        &provider,
        PROGRAM_ID,
        EmitMixedEventAccounts {
            signer: provider.payer.address(),
            state,
        },
    )
    .send_and_confirm()
    .expect("emit_mixed_event should succeed");

    let events = meta
        .parse_events_zero_copy::<MixedEvent>()
        .expect("events should parse");
    assert_eq!(events.len(), 1, "exactly one MixedEvent expected");
    assert_eq!(events[0].tag, 9);
    let amount = events[0].amount;
    assert_eq!(amount, 123_456_789);
    assert!(matches!(events[0].color.get(), Ok(Color::Blue)));
}

#[test]
fn instruction_data_matches_the_zero_copy_program_byte_for_byte() {
    let (provider, state) = setup();
    assert_matches_zero_copy_golden(
        &dyn_args_instruction(&provider, state).build_instruction().data,
        "dyn_args_ix.hex",
    );
    assert_matches_zero_copy_golden(
        &text_lists_instruction(&provider, state).build_instruction().data,
        "text_lists_ix.hex",
    );
    assert_matches_zero_copy_golden(
        &enum_lists_instruction(&provider, state).build_instruction().data,
        "enum_lists_ix.hex",
    );
}
