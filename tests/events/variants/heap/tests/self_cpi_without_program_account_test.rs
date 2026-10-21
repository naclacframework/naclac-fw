use naclac_client::*;
use events_borsh_client::{
    instructions::{build_emit_via_self_cpi_no_program, EmitViaSelfCpiNoProgramAccounts},
    types::PROGRAM_ID,
};

/// The runtime requires the callee program among the outer instruction's
/// accounts, so a self-CPI without it fails: this is why `#[event_cpi]`
/// injects the `program` account.
#[test]
fn self_cpi_without_the_program_account_is_rejected() {
    let payer = load_node_wallet().expect("Failed to load local Solana keypair");
    let provider =
        NaclacProvider::new("litesvm", payer).expect("Failed to construct NaclacProvider");
    let event_authority = Address::find_program_address(&[b"__event_authority"], &PROGRAM_ID).0;

    let error = build_emit_via_self_cpi_no_program(
        &provider,
        PROGRAM_ID,
        EmitViaSelfCpiNoProgramAccounts { event_authority },
    )
    .send_and_confirm()
    .expect_err("a self-CPI without the `program` account must fail");
    assert!(format!("{error:?}").contains("MissingAccount"), "{error:?}");
}
