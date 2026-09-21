use naclac_client::*;
use single_file_layout_client::{
    fetch_counter, get_counter_pda,
    instructions::{
        build_increment_counter, build_init_counter, IncrementCounterAccounts,
        InitCounterAccounts,
    },
    types::PROGRAM_ID,
};

/// Proves `naclac-syn`'s source discovery (a plain recursive walk of every
/// `.rs` file under `src/` â€” see `naclac-syn/src/lib.rs`'s
/// `parse_workspace_program`) doesn't require the `components/`/
/// `instructions/` folder split every other test/example uses: the
/// `#[component]`, both `#[derive(Accounts)]` structs, and both
/// `#[instruction]` handlers here all live directly in `lib.rs`, and the
/// generated client SDK (including the `get_counter_pda` PDA helper, which
/// specifically depends on the IDL correctly resolving the `seeds`/`bump`
/// on the `Counter` component) still comes out correct and works on-chain,
/// exactly like the split layout.
#[test]
fn single_file_program_generates_a_working_sdk() {
    let payer = load_node_wallet().expect("Failed to load local Solana keypair");
    let provider = NaclacProvider::new("litesvm", payer).expect("Failed to construct NaclacProvider");

    let (counter_pda, _bump) = get_counter_pda(&PROGRAM_ID);

    build_init_counter(
        &provider,
        PROGRAM_ID,
        InitCounterAccounts {
            payer: provider.payer.address(),
            counter: counter_pda,
            system_program: Address::default(),
        },
    )
    .send_and_confirm()
    .expect("init_counter should succeed");

    let counter = fetch_counter(&provider, &counter_pda)
        .expect("counter should be readable after init");
    assert_eq!(counter.value, 0);

    build_increment_counter(
        &provider,
        PROGRAM_ID,
        IncrementCounterAccounts { counter: counter_pda },
    )
    .send_and_confirm()
    .expect("increment_counter should succeed");

    let counter = fetch_counter(&provider, &counter_pda)
        .expect("counter should be readable after increment");
    assert_eq!(counter.value, 1);
}
