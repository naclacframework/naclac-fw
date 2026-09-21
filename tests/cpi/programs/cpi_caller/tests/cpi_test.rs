use naclac_client::*;
use cpi_caller_client::{
    get_caller_authority_pda,
    instructions::{
        build_call_authorized_increment, build_call_setup_counter, build_call_system_transfer,
        build_init_caller_authority, CallAuthorizedIncrementAccounts, CallSetupCounterAccounts,
        CallSystemTransferAccounts, InitCallerAuthorityAccounts,
    },
    types::PROGRAM_ID as CALLER_PROGRAM_ID,
};
use cpi_callee_client::{fetch_counter, get_counter_pda, types::PROGRAM_ID as CALLEE_PROGRAM_ID};

fn setup() -> NaclacProvider {
    let payer = load_node_wallet().expect("Failed to load local Solana keypair");
    NaclacProvider::new("litesvm", payer).expect("Failed to construct NaclacProvider")
}

/// The full cross-program CPI story in one pass:
/// 1. real CPI to the System Program (`call_system_transfer`)
/// 2. real cross-program CPI (unsigned) to a *second*, separately-deployed
///    naclac program, via that program's auto-generated client SDK
///    (`call_setup_counter` -> `cpi_callee`'s `init_counter`)
/// 3. real cross-program CPI *signed with PDA seeds* â€” `cpi_callee`'s
///    `authorized_increment` requires its `authority` to sign, and that
///    authority is `cpi_caller`'s own PDA, so the CPI must be
///    `invoke_signed` with that PDA's exact seeds
///    (`call_authorized_increment`)
#[test]
fn cross_program_cpi_works_end_to_end() {
    let provider = setup();

    let (caller_authority_pda, _bump) = get_caller_authority_pda(&CALLER_PROGRAM_ID);
    let (counter_pda, _counter_bump) = get_counter_pda(&CALLEE_PROGRAM_ID);

    build_init_caller_authority(
        &provider,
        CALLER_PROGRAM_ID,
        InitCallerAuthorityAccounts {
            payer: provider.payer.address(),
            caller_authority: caller_authority_pda,
            system_program: Address::default(),
        },
    )
    .send_and_confirm()
    .expect("init_caller_authority should succeed");

    // 1. Real System Program CPI.
    build_call_system_transfer(
        &provider,
        CALLER_PROGRAM_ID,
        1_000_000,
        CallSystemTransferAccounts {
            payer: provider.payer.address(),
            caller_authority: caller_authority_pda,
            system_program: Address::default(),
        },
    )
    .send_and_confirm()
    .expect("call_system_transfer (real System Program CPI) should succeed");

    let caller_authority_account = provider
        .get_account(&caller_authority_pda)
        .expect("caller_authority account should exist");
    assert!(
        caller_authority_account.lamports > 0,
        "the System Program transfer must have actually moved lamports"
    );

    // 2. Real cross-program CPI (unsigned) to a second, separately-deployed
    //    naclac program via its generated client SDK.
    build_call_setup_counter(
        &provider,
        CALLER_PROGRAM_ID,
        CallSetupCounterAccounts {
            payer: provider.payer.address(),
            counter: counter_pda,
            caller_authority: caller_authority_pda,
            callee_program: CALLEE_PROGRAM_ID,
            system_program: Address::default(),
        },
    )
    .send_and_confirm()
    .expect("call_setup_counter (cross-program CPI to cpi_callee) should succeed");

    let counter_after_setup =
        fetch_counter(&provider, &counter_pda).expect("counter should be readable after setup");
    assert_eq!(counter_after_setup.value, 0);
    assert_eq!(counter_after_setup.authority, caller_authority_pda.into());

    // 3. Real cross-program CPI, signed with cpi_caller's own PDA seeds.
    build_call_authorized_increment(
        &provider,
        CALLER_PROGRAM_ID,
        CallAuthorizedIncrementAccounts {
            counter: counter_pda,
            caller_authority: caller_authority_pda,
            callee_program: CALLEE_PROGRAM_ID,
        },
    )
    .send_and_confirm()
    .expect(
        "call_authorized_increment (signed cross-program CPI with PDA seeds) should succeed",
    );

    let counter_after_increment = fetch_counter(&provider, &counter_pda)
        .expect("counter should be readable after the signed CPI");
    assert_eq!(
        counter_after_increment.value, 1,
        "the signed cross-program CPI must have actually run cpi_callee's authorized_increment"
    );
}
