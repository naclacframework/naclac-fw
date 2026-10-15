//! Discriminator checking on a `#[account(mut)]` slot: `touch_config`'s
//! `config` has no seeds or address constraint, so a same-program account of
//! another type (a Vault) can only be rejected by the discriminator check.

use naclac_client::*;
use discriminator_zc_client::{
    instructions::{
        build_init_config, build_init_vault, build_touch_config, InitConfigAccounts,
        InitVaultAccounts, TouchConfigAccounts,
    },
    get_config_pda, get_vault_pda,
    types::PROGRAM_ID,
};

fn assert_custom_code(result: Result<NaclacTransactionMetadata, NaclacClientError>, expected: u32) {
    match result {
        Err(NaclacClientError::TransactionFailed {
            instruction_err: InstructionError::Custom(code),
            ..
        }) => {
            assert_eq!(code, expected, "wrong custom error code");
        }
        other => panic!("expected a Custom({expected}) transaction failure, got {other:?}"),
    }
}

#[test]
fn rejects_type_confused_account_in_mut_config_slot() {
    let payer = load_node_wallet().expect("Failed to load local Solana keypair");
    let provider = NaclacProvider::new("litesvm", payer).expect("Failed to construct NaclacProvider");

    let (vault_pda, _bump) = get_vault_pda(&PROGRAM_ID);

    build_init_vault(
        &provider,
        PROGRAM_ID,
        InitVaultAccounts {
            payer: provider.payer.address(),
            vault: vault_pda,
            system_program: Address::default(),
        },
    )
    .send_and_confirm()
    .expect("init_vault should succeed");

    let result = build_touch_config(
        &provider,
        PROGRAM_ID,
        TouchConfigAccounts {
            caller: provider.payer.address(),
            config: vault_pda,
        },
    )
    .send_and_confirm();

    // `config` is field index 1: 10000 + 1*100 + InvalidAccountDiscriminator (22).
    assert_custom_code(result, 10122);
}

#[test]
fn accepts_correctly_typed_config_in_mut_config_slot() {
    let payer = load_node_wallet().expect("Failed to load local Solana keypair");
    let provider = NaclacProvider::new("litesvm", payer).expect("Failed to construct NaclacProvider");

    let (config_pda, _bump) = get_config_pda(&PROGRAM_ID);

    build_init_config(
        &provider,
        PROGRAM_ID,
        InitConfigAccounts {
            payer: provider.payer.address(),
            config: config_pda,
            system_program: Address::default(),
        },
    )
    .send_and_confirm()
    .expect("init_config should succeed");

    build_touch_config(
        &provider,
        PROGRAM_ID,
        TouchConfigAccounts {
            caller: provider.payer.address(),
            config: config_pda,
        },
    )
    .send_and_confirm()
    .expect("touch_config on a real Config account should succeed");
}
