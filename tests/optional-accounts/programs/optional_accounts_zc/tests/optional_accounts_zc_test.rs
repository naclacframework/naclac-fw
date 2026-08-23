use naclac_client::*;
use optional_accounts_zc_client::{
    fetch_thing,
    get_thing_a_pda, get_thing_b_pda, get_optional_thing_pda,
    instructions::{
        build_init_thing_a, build_init_thing_b, build_touch_optional, build_touch_two_optional,
        build_init_optional_thing, build_close_optional_thing, build_realloc_optional_thing,
        build_touch_boxed_optional,
        InitThingAAccounts, InitThingBAccounts, TouchOptionalAccounts, TouchTwoOptionalAccounts,
        InitOptionalThingAccounts, CloseOptionalThingAccounts, ReallocOptionalThingAccounts,
        TouchBoxedOptionalAccounts,
    },
    types::PROGRAM_ID,
};

fn load_program(provider: &NaclacProvider) {
    let mut so_path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    so_path.pop(); // programs
    so_path.pop(); // optional-accounts workspace root
    so_path.push("target/deploy/optional_accounts_zc.so");

    provider
        .add_program(&PROGRAM_ID, so_path.to_str().unwrap())
        .expect("Failed to load optional_accounts_zc program binary");
}

fn setup() -> NaclacProvider {
    let payer = load_node_wallet().expect("Failed to load local Solana keypair");
    let provider = NaclacProvider::new("litesvm", payer);
    load_program(&provider);
    provider
}

fn init_thing_a(provider: &NaclacProvider) -> Address {
    let (thing_a_pda, _bump) = get_thing_a_pda(&PROGRAM_ID);
    build_init_thing_a(
        provider,
        PROGRAM_ID,
        InitThingAAccounts {
            payer: provider.payer.address(),
            thing_a: thing_a_pda,
            system_program: Address::default(),
        },
    )
    .send_and_confirm()
    .expect("init_thing_a should succeed");
    thing_a_pda
}

fn init_thing_b(provider: &NaclacProvider) -> Address {
    let (thing_b_pda, _bump) = get_thing_b_pda(&PROGRAM_ID);
    build_init_thing_b(
        provider,
        PROGRAM_ID,
        InitThingBAccounts {
            payer: provider.payer.address(),
            thing_b: thing_b_pda,
            system_program: Address::default(),
        },
    )
    .send_and_confirm()
    .expect("init_thing_b should succeed");
    thing_b_pda
}

/// The present case: a real `Thing` account passed through the sentinel-gated
/// `Option<Account<Thing>>` field must be loaded, validated, and mutated
/// exactly like a non-optional field would be.
#[test]
fn touch_optional_increments_when_present() {
    let provider = setup();
    let thing_a_pda = init_thing_a(&provider);

    build_touch_optional(
        &provider,
        PROGRAM_ID,
        TouchOptionalAccounts {
            thing: Some(thing_a_pda),
        },
    )
    .send_and_confirm()
    .expect("touch_optional with a real account should succeed");

    let thing = fetch_thing(&provider, &thing_a_pda).expect("thing_a should be readable");
    assert_eq!(thing.value, 1);
}

/// The absent case: passing `None` must produce the sentinel (the program's
/// own address) in that slot, be recognized as absent by `load_and_validate`,
/// and skip the field entirely — not fail, not misindex later accounts (there
/// are none after it here, but `touch_two_optional` below covers ordering).
#[test]
fn touch_optional_is_noop_when_absent() {
    let provider = setup();

    let result = build_touch_optional(&provider, PROGRAM_ID, TouchOptionalAccounts { thing: None })
        .send_and_confirm();

    assert!(
        result.is_ok(),
        "touch_optional with no account should succeed as a no-op: {:?}",
        result.err()
    );
}

/// A real, non-`Thing` account (the payer's own wallet — no discriminator,
/// wrong owner) passed into the optional slot must still be rejected by the
/// same validation a non-optional `Account<Thing>` field would apply.
/// Optionality only gates *whether* loading happens, not *how thoroughly*.
#[test]
fn touch_optional_rejects_wrong_account_when_present() {
    let provider = setup();

    let result = build_touch_optional(
        &provider,
        PROGRAM_ID,
        TouchOptionalAccounts {
            thing: Some(provider.payer.address()),
        },
    )
    .send_and_confirm();

    assert!(
        result.is_err(),
        "touch_optional with a real but invalid account should be rejected"
    );
}

/// Both optional `mut` accounts present: both get incremented independently.
#[test]
fn touch_two_optional_both_present_increments_both() {
    let provider = setup();
    let thing_a_pda = init_thing_a(&provider);
    let thing_b_pda = init_thing_b(&provider);

    build_touch_two_optional(
        &provider,
        PROGRAM_ID,
        TouchTwoOptionalAccounts {
            thing_a: Some(thing_a_pda),
            thing_b: Some(thing_b_pda),
        },
    )
    .send_and_confirm()
    .expect("touch_two_optional with both accounts present should succeed");

    let thing_a = fetch_thing(&provider, &thing_a_pda).expect("thing_a should be readable");
    let thing_b = fetch_thing(&provider, &thing_b_pda).expect("thing_b should be readable");
    assert_eq!(thing_a.value, 1);
    assert_eq!(thing_b.value, 1);
}

/// The `MUT_MASK` regression case: two independent optional `mut` fields,
/// both omitted in the same call, both carrying the identical sentinel
/// address (the program's own). Before the `MUT_MASK` fix, this would have
/// falsely tripped the duplicate-mutable-account guard — two `mut` slots
/// sharing one address looks exactly like a real collision unless optional
/// fields are excluded from the mask.
#[test]
fn touch_two_optional_both_absent_does_not_trigger_duplicate_mutable_error() {
    let provider = setup();

    let result = build_touch_two_optional(
        &provider,
        PROGRAM_ID,
        TouchTwoOptionalAccounts {
            thing_a: None,
            thing_b: None,
        },
    )
    .send_and_confirm();

    assert!(
        result.is_ok(),
        "two omitted optional mut accounts must not be treated as a duplicate-mutable collision: {:?}",
        result.err()
    );
}

/// Mixed case: one optional field present, the other absent, in the same
/// call — confirms the sentinel comparison is per-slot, not all-or-nothing.
#[test]
fn touch_two_optional_mixed_presence_only_touches_the_present_one() {
    let provider = setup();
    let thing_a_pda = init_thing_a(&provider);

    build_touch_two_optional(
        &provider,
        PROGRAM_ID,
        TouchTwoOptionalAccounts {
            thing_a: Some(thing_a_pda),
            thing_b: None,
        },
    )
    .send_and_confirm()
    .expect("touch_two_optional with only thing_a present should succeed");

    let thing_a = fetch_thing(&provider, &thing_a_pda).expect("thing_a should be readable");
    assert_eq!(thing_a.value, 1);
}

/// `init` on a seeded `Option<T>` field: a caller passing a real,
/// not-yet-existing PDA must have it created and initialized exactly like a
/// non-optional `init` would — `init_cpi.rs`'s codegen runs before `self`
/// exists, already inside the sentinel's `Some` branch, so this needed no
/// codegen changes, only removing the parser-level rejection.
#[test]
fn init_optional_thing_creates_when_present() {
    let provider = setup();
    let (optional_thing_pda, _bump) = get_optional_thing_pda(&PROGRAM_ID);

    build_init_optional_thing(
        &provider,
        PROGRAM_ID,
        InitOptionalThingAccounts {
            payer: provider.payer.address(),
            optional_thing: Some(optional_thing_pda),
            system_program: Address::default(),
        },
    )
    .send_and_confirm()
    .expect("init_optional_thing with a real account should succeed and create it");

    let optional_thing =
        fetch_thing(&provider, &optional_thing_pda).expect("optional_thing should be readable");
    assert_eq!(optional_thing.value, 42);
}

/// `init` on an absent optional field: the sentinel check gates everything,
/// including `#init_logic` — so passing `None` must skip account creation
/// entirely, not attempt to create an account at the program's own address.
#[test]
fn init_optional_thing_skips_when_absent() {
    let provider = setup();
    let (optional_thing_pda, _bump) = get_optional_thing_pda(&PROGRAM_ID);

    let result = build_init_optional_thing(
        &provider,
        PROGRAM_ID,
        InitOptionalThingAccounts {
            payer: provider.payer.address(),
            optional_thing: None,
            system_program: Address::default(),
        },
    )
    .send_and_confirm();

    assert!(
        result.is_ok(),
        "init_optional_thing with no account should succeed as a no-op: {:?}",
        result.err()
    );
    assert!(
        provider.get_account(&optional_thing_pda).is_err(),
        "no account should have been created at the PDA when the field was omitted"
    );
}

/// `close` on a present `Option<T>` field: `close_account.rs` runs in
/// `teardown()`, after `self` exists, so this needed a real fix (unlike
/// `init`) — bound to a local `__target` inside `if let Some(...)`.
#[test]
fn close_optional_thing_closes_when_present() {
    let provider = setup();
    let (optional_thing_pda, _bump) = get_optional_thing_pda(&PROGRAM_ID);

    build_init_optional_thing(
        &provider,
        PROGRAM_ID,
        InitOptionalThingAccounts {
            payer: provider.payer.address(),
            optional_thing: Some(optional_thing_pda),
            system_program: Address::default(),
        },
    )
    .send_and_confirm()
    .expect("init_optional_thing should succeed");

    build_close_optional_thing(
        &provider,
        PROGRAM_ID,
        CloseOptionalThingAccounts {
            payer: provider.payer.address(),
            optional_thing: Some(optional_thing_pda),
        },
    )
    .send_and_confirm()
    .expect("close_optional_thing with a real account should succeed");
}

/// `close` on an absent optional field: nothing to close, must succeed as a
/// no-op rather than fail trying to close a sentinel slot.
#[test]
fn close_optional_thing_is_noop_when_absent() {
    let provider = setup();

    let result = build_close_optional_thing(
        &provider,
        PROGRAM_ID,
        CloseOptionalThingAccounts {
            payer: provider.payer.address(),
            optional_thing: None,
        },
    )
    .send_and_confirm();

    assert!(
        result.is_ok(),
        "close_optional_thing with no account should succeed as a no-op: {:?}",
        result.err()
    );
}

/// `realloc` on a present `Option<T>` field: same "needed a real fix" shape
/// as `close` — resizes exactly like a non-optional `realloc` would.
#[test]
fn realloc_optional_thing_resizes_when_present() {
    let provider = setup();
    let (optional_thing_pda, _bump) = get_optional_thing_pda(&PROGRAM_ID);

    build_init_optional_thing(
        &provider,
        PROGRAM_ID,
        InitOptionalThingAccounts {
            payer: provider.payer.address(),
            optional_thing: Some(optional_thing_pda),
            system_program: Address::default(),
        },
    )
    .send_and_confirm()
    .expect("init_optional_thing should succeed");

    build_realloc_optional_thing(
        &provider,
        PROGRAM_ID,
        24,
        ReallocOptionalThingAccounts {
            payer: provider.payer.address(),
            optional_thing: Some(optional_thing_pda),
            system_program: Address::default(),
        },
    )
    .send_and_confirm()
    .expect("realloc_optional_thing with a real account should succeed");

    let account = provider
        .get_account(&optional_thing_pda)
        .expect("optional_thing account should still exist after realloc");
    assert_eq!(account.data.len(), 24);
}

/// `realloc` on an absent optional field: nothing to resize, must succeed
/// as a no-op rather than fail trying to resize a sentinel slot.
#[test]
fn realloc_optional_thing_is_noop_when_absent() {
    let provider = setup();

    let result = build_realloc_optional_thing(
        &provider,
        PROGRAM_ID,
        24,
        ReallocOptionalThingAccounts {
            payer: provider.payer.address(),
            optional_thing: None,
            system_program: Address::default(),
        },
    )
    .send_and_confirm();

    assert!(
        result.is_ok(),
        "realloc_optional_thing with no account should succeed as a no-op: {:?}",
        result.err()
    );
}

/// `Option<Box<Account<Thing>>>` — Box *inside* Option. Already works with
/// no macro changes: `type_classify::is_option_account` only looks at the
/// outermost segment (`Option`), so the unwrapped field type becomes
/// `Box<Account<Thing>>` as-is, and naclac-core's existing blanket
/// `NaclacAccount`/`ToAddress`/`ToAccountInfo` impls for `Box<T>` make every
/// call site the sentinel codegen already uses work transparently.
#[test]
fn touch_boxed_optional_increments_when_present() {
    let provider = setup();
    let thing_a_pda = init_thing_a(&provider);

    build_touch_boxed_optional(
        &provider,
        PROGRAM_ID,
        TouchBoxedOptionalAccounts {
            thing: Some(thing_a_pda),
        },
    )
    .send_and_confirm()
    .expect("touch_boxed_optional with a real account should succeed");

    let thing = fetch_thing(&provider, &thing_a_pda).expect("thing_a should be readable");
    assert_eq!(thing.value, 1);
}

/// Absent case for the boxed optional field: same sentinel no-op behavior
/// as the unboxed case.
#[test]
fn touch_boxed_optional_is_noop_when_absent() {
    let provider = setup();

    let result = build_touch_boxed_optional(
        &provider,
        PROGRAM_ID,
        TouchBoxedOptionalAccounts { thing: None },
    )
    .send_and_confirm();

    assert!(
        result.is_ok(),
        "touch_boxed_optional with no account should succeed as a no-op: {:?}",
        result.err()
    );
}
