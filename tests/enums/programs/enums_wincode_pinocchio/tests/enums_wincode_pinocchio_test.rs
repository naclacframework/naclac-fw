//! End-to-end LiteSVM tests for plain enum fields in a Wincode program: every
//! write and read goes through the generated Rust client against the compiled
//! program, and the compute units each operation costs are printed from the
//! real execution metadata (run with `--nocapture`).

use enums_wincode_pinocchio_client::{
    get_state_pda,
    instructions::*,
    types::{Action, Color, ModeArgs, StateChanged, PROGRAM_ID},
    State,
};
use naclac_client::*;

const DISCRIMINATOR_LEN: usize = 8;
const COLOR_TAG_OFFSET: usize = DISCRIMINATOR_LEN + 1 + 32;

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
    enums_wincode_pinocchio_client::fetch_state(provider, state).expect("state should decode")
}

fn set_action(provider: &NaclacProvider, state: Address, action: Action) {
    build_set_action(
        provider,
        PROGRAM_ID,
        action,
        SetActionAccounts {
            signer: provider.payer.address(),
            state,
        },
    )
    .send_and_confirm()
    .expect("set_action should succeed");
}

fn set_color(provider: &NaclacProvider, state: Address, color: Color) {
    build_set_color(
        provider,
        PROGRAM_ID,
        color,
        SetColorAccounts {
            signer: provider.payer.address(),
            state,
        },
    )
    .send_and_confirm()
    .expect("set_color should succeed");
}

#[test]
fn init_writes_the_chosen_variants_and_first_variants_elsewhere() {
    let (provider, state) = setup();
    let s = read_state(&provider, &state);

    assert!(matches!(s.color, Color::Green));
    assert!(matches!(s.action, Action::SetFee { fee_bps: 25 }));
    assert!(s.history.iter().all(|c| matches!(c, Color::Red)));
    assert!(s.maybe_color.is_none());
    assert!(matches!(s.limits.mode, Color::Red));
    assert_eq!(s.counter, 0);
}

#[test]
fn set_action_round_trips_every_variant_shape() {
    let (provider, state) = setup();
    let to = Address::new_from_array([9; 32]);

    set_action(&provider, state, Action::Transfer { amount: 777, to });
    match read_state(&provider, &state).action {
        Action::Transfer { amount, to: got } => {
            assert_eq!(amount, 777);
            assert_eq!(got, to);
        }
        other => panic!("expected Transfer, got {other:?}"),
    }

    set_action(&provider, state, Action::Paint(Color::Blue));
    assert!(matches!(
        read_state(&provider, &state).action,
        Action::Paint(Color::Blue)
    ));

    set_action(&provider, state, Action::Pause);
    assert!(matches!(read_state(&provider, &state).action, Action::Pause));
}

#[test]
fn set_color_round_trips_a_non_contiguous_discriminant() {
    let (provider, state) = setup();
    set_color(&provider, state, Color::Blue);
    assert!(matches!(read_state(&provider, &state).color, Color::Blue));
}

#[test]
fn the_color_is_stored_as_its_declared_discriminant_byte() {
    let (provider, state) = setup();
    set_color(&provider, state, Color::Blue);
    let data = provider.get_account_data(&state).unwrap();
    assert_eq!(data[COLOR_TAG_OFFSET], 7);
}

#[test]
fn an_invalid_enum_tag_in_the_account_is_rejected_on_chain() {
    let (provider, state) = setup();
    let mut data = provider.get_account_data(&state).unwrap();
    let owner = provider.get_account(&state).unwrap().owner;
    let lamports = provider.get_balance(&state).unwrap();

    data[COLOR_TAG_OFFSET] = 5;
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
                instruction_err: InstructionError::Custom(10123),
                ..
            })
        ),
        "an enum tag with no variant must fail with DeserializationFailed, got {result:?}"
    );
}

#[test]
fn an_enum_in_instruction_args_reaches_the_program_intact() {
    let (provider, state) = setup();
    build_set_mode_with_args(
        &provider,
        PROGRAM_ID,
        ModeArgs {
            mode: Color::Blue,
            amount: 9,
        },
        SetModeWithArgsAccounts {
            signer: provider.payer.address(),
            state,
        },
    )
    .send_and_confirm()
    .expect("set_mode_with_args should succeed");

    let s = read_state(&provider, &state);
    assert!(matches!(s.color, Color::Blue));
    assert_eq!(s.counter, 9);
}

#[test]
fn an_event_carrying_enums_decodes_to_the_emitted_values() {
    let (provider, state) = setup();
    let to = Address::new_from_array([3; 32]);
    set_action(&provider, state, Action::Transfer { amount: 42, to });
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
        .parse_events_wincode::<StateChanged>()
        .expect("events should parse");
    assert_eq!(events.len(), 1, "exactly one StateChanged event expected");
    assert!(matches!(events[0].color, Color::Blue));
    match &events[0].action {
        Action::Transfer { amount, to: got } => {
            assert_eq!(*amount, 42);
            assert_eq!(*got, to);
        }
        other => panic!("expected Transfer, got {other:?}"),
    }
}

fn assert_golden(name: &str, bytes: &[u8]) {
    let hex: String = bytes.iter().map(|b| format!("{b:02x}")).collect();
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../golden")
        .join(name);
    if std::env::var_os("UPDATE_GOLDEN").is_some() || !path.exists() {
        std::fs::write(&path, &hex).unwrap();
    }
    let expected = std::fs::read_to_string(&path).expect("golden fixture must exist");
    assert_eq!(hex, expected.trim());
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
    let admin_at = DISCRIMINATOR_LEN + 1;
    data[admin_at..admin_at + 32].fill(0);
    data[DISCRIMINATOR_LEN] = 0;
    assert_golden("wincode_state.hex", &data);
}

#[test]
fn the_instruction_data_matches_the_golden_fixtures_shared_with_the_typescript_client() {
    let (provider, state) = setup();
    let signer = provider.payer.address();

    let set_fee = build_set_action(
        &provider,
        PROGRAM_ID,
        Action::SetFee { fee_bps: 321 },
        SetActionAccounts { signer, state },
    )
    .build_instruction()
    .data;
    assert_golden("wincode_set_fee_ix.hex", &set_fee);

    let set_mode = build_set_mode_with_args(
        &provider,
        PROGRAM_ID,
        ModeArgs {
            mode: Color::Blue,
            amount: 9,
        },
        SetModeWithArgsAccounts { signer, state },
    )
    .build_instruction()
    .data;
    assert_golden("wincode_set_mode_ix.hex", &set_mode);
}

#[test]
fn compute_units_per_operation() {
    macro_rules! cost {
        ($builder:ident, $accounts:ident) => {{
            let (provider, state) = setup();
            $builder(
                &provider,
                PROGRAM_ID,
                $accounts {
                    signer: provider.payer.address(),
                    state,
                },
            )
            .send_and_confirm()
            .expect("bench instruction should succeed")
            .compute_units_consumed
        }};
    }

    let rows = [
        ("noop (baseline)", cost!(build_bench_noop, BenchNoopAccounts)),
        (
            "u8 field = const",
            cost!(build_bench_u8_set_const, BenchU8SetConstAccounts),
        ),
        (
            "Color = const",
            cost!(build_bench_color_set_const, BenchColorSetConstAccounts),
        ),
        (
            "match Color",
            cost!(build_bench_color_get, BenchColorGetAccounts),
        ),
        (
            "Action = Transfer",
            cost!(build_bench_action_set_const, BenchActionSetConstAccounts),
        ),
        (
            "match Action",
            cost!(build_bench_action_get, BenchActionGetAccounts),
        ),
    ];

    println!("\n{:<32} {:>8} {:>10}", "operation", "CU", "vs noop");
    let baseline = rows[0].1;
    for (name, cu) in rows {
        println!("{:<32} {:>8} {:>+10}", name, cu, cu as i64 - baseline as i64);
    }
}
