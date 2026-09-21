use naclac_client::*;
use stack_safety_client::{
    fetch_big_data, fetch_small_data,
    get_big_pda, get_small_pda,
    instructions::{
        build_close_big, build_init_big, build_init_small, build_touch_big, build_touch_small,
        CloseBigAccounts, InitBigAccounts, InitSmallAccounts, TouchBigAccounts,
        TouchSmallAccounts,
    },
    types::PROGRAM_ID,
};

fn setup() -> NaclacProvider {
    let payer = load_node_wallet().expect("Failed to load local Solana keypair");
    NaclacProvider::new("litesvm", payer).expect("Failed to construct NaclacProvider")
}

/// The ordinary path: a component well under both stack budgets, unboxed.
/// Confirms boxing is opt-in, not mandatory everywhere.
#[test]
fn small_unboxed_field_inits_and_mutates() {
    let provider = setup();
    let (small_pda, _bump) = get_small_pda(&PROGRAM_ID);

    build_init_small(
        &provider,
        PROGRAM_ID,
        InitSmallAccounts {
            payer: provider.payer.address(),
            small: small_pda,
            system_program: Address::default(),
        },
    )
    .send_and_confirm()
    .expect("init_small should succeed");

    let before = fetch_small_data(&provider, &small_pda).expect("small should be readable after init");
    assert_eq!(before.value, 0);

    build_touch_small(
        &provider,
        PROGRAM_ID,
        TouchSmallAccounts { small: small_pda },
    )
    .send_and_confirm()
    .expect("touch_small should succeed");

    let after = fetch_small_data(&provider, &small_pda).expect("small should still be readable");
    assert_eq!(after.value, 1);
}

/// The actual mechanism under test: `BigData`'s 300-byte payload alone puts
/// `Account<BigData>` over the per-field stack budget, so every field of
/// this type in the real program is `Box<Account<BigData>>` â€” this isn't a
/// stand-in case, boxing here is load-bearing (an unboxed version of this
/// exact program fails to compile, per `tests/stack-safety/compile-fail/`).
/// `init`/mutation/`.address()` must all work exactly like an unboxed field.
#[test]
fn big_boxed_field_inits_and_mutates_and_address_works() {
    let provider = setup();
    let (big_pda, _bump) = get_big_pda(&PROGRAM_ID);

    build_init_big(
        &provider,
        PROGRAM_ID,
        InitBigAccounts {
            payer: provider.payer.address(),
            big: big_pda,
            system_program: Address::default(),
        },
    )
    .send_and_confirm()
    .expect("init_big should succeed");

    let before = fetch_big_data(&provider, &big_pda).expect("big should be readable after init");
    assert_eq!(before.payload[0], 0);

    build_touch_big(&provider, PROGRAM_ID, TouchBigAccounts { big: big_pda })
        .send_and_confirm()
        .expect("touch_big should succeed");

    let after = fetch_big_data(&provider, &big_pda).expect("big should still be readable");
    assert_eq!(
        after.payload[0], 1,
        "DerefMut through Box<Account<T>> must genuinely persist the mutation back to the account"
    );
}

/// `close`'s lamport-drain/reassign logic operates on the field's raw
/// `AccountInfo` â€” a different blanket impl (`ToAccountInfo`) than the
/// `Deref`/`DerefMut`/`ToAddress` path the mutation test above exercises.
/// Confirms that side is also fully transparent through the Box.
#[test]
fn big_boxed_field_close_drains_lamports() {
    let provider = setup();
    let (big_pda, _bump) = get_big_pda(&PROGRAM_ID);

    build_init_big(
        &provider,
        PROGRAM_ID,
        InitBigAccounts {
            payer: provider.payer.address(),
            big: big_pda,
            system_program: Address::default(),
        },
    )
    .send_and_confirm()
    .expect("init_big should succeed");

    let payer_lamports_before = provider
        .get_account(&provider.payer.address())
        .expect("payer account should exist")
        .lamports;

    build_close_big(
        &provider,
        PROGRAM_ID,
        CloseBigAccounts {
            payer: provider.payer.address(),
            big: big_pda,
        },
    )
    .send_and_confirm()
    .expect("close_big should succeed");

    let payer_lamports_after = provider
        .get_account(&provider.payer.address())
        .expect("payer account should exist")
        .lamports;
    assert!(
        payer_lamports_after > payer_lamports_before,
        "closing the boxed big account must transfer its lamports to the payer"
    );
}
