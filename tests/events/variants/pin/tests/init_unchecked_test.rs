use naclac_client::*;
use events_client::{
    instructions::{build_init_counter_unchecked, InitCounterUncheckedAccounts},
    types::PROGRAM_ID,
};

#[test]
fn init_unchecked_creates_the_account_through_the_raw_create_account_cpi() {
    let payer = load_node_wallet().expect("Failed to load local Solana keypair");
    let provider =
        NaclacProvider::new("litesvm", payer).expect("Failed to construct NaclacProvider");
    let counter = Address::find_program_address(&[b"counter"], &PROGRAM_ID).0;

    build_init_counter_unchecked(
        &provider,
        PROGRAM_ID,
        InitCounterUncheckedAccounts {
            payer: provider.payer.address(),
            counter,
            system_program: Address::default(),
        },
    )
    .send_and_confirm()
    .expect("init_unchecked must create the account");

    let account = provider.get_account(&counter).expect("counter account must exist");
    assert_eq!(account.owner, PROGRAM_ID);
}
