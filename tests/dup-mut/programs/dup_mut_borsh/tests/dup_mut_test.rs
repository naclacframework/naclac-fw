use naclac_client::*;
use dup_mut_borsh_client::{
    instructions::{
        build_init_vault_a, build_init_vault_b, build_touch_pair_no_alias,
        build_touch_pair_with_alias, InitVaultAAccounts, InitVaultBAccounts,
        TouchPairNoAliasAccounts, TouchPairWithAliasAccounts,
    },
    get_vault_a_pda, get_vault_b_pda,
    types::PROGRAM_ID,
};

fn load_program(provider: &NaclacProvider) {
    let mut so_path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    so_path.pop(); // programs
    so_path.pop(); // dup-mut workspace root
    so_path.push("target/deploy/dup_mut_borsh.so");

    provider
        .add_program(&PROGRAM_ID, so_path.to_str().unwrap())
        .expect("Failed to load dup_mut_borsh program binary");
}

fn setup() -> (NaclacProvider, Address, Address) {
    let payer = load_node_wallet().expect("Failed to load local Solana keypair");
    let provider = NaclacProvider::new("litesvm", payer);
    load_program(&provider);

    let (vault_a_pda, _bump) = get_vault_a_pda(&PROGRAM_ID);
    let (vault_b_pda, _bump) = get_vault_b_pda(&PROGRAM_ID);

    build_init_vault_a(
        &provider,
        PROGRAM_ID,
        InitVaultAAccounts {
            payer: provider.payer.address(),
            vault_a: vault_a_pda,
            system_program: Address::default(),
        },
    )
    .send_and_confirm()
    .expect("init_vault_a should succeed");

    build_init_vault_b(
        &provider,
        PROGRAM_ID,
        InitVaultBAccounts {
            payer: provider.payer.address(),
            vault_b: vault_b_pda,
            system_program: Address::default(),
        },
    )
    .send_and_confirm()
    .expect("init_vault_b should succeed");

    (provider, vault_a_pda, vault_b_pda)
}

#[test]
fn distinct_accounts_are_accepted() {
    let (provider, vault_a_pda, vault_b_pda) = setup();

    let result = build_touch_pair_no_alias(
        &provider,
        PROGRAM_ID,
        TouchPairNoAliasAccounts {
            a: vault_a_pda,
            b: vault_b_pda,
        },
    )
    .send_and_confirm();

    assert!(
        result.is_ok(),
        "two distinct mutable accounts should be accepted: {:?}",
        result.err()
    );
}

#[test]
fn duplicate_mutable_account_without_alias_is_rejected() {
    let (provider, vault_a_pda, _vault_b_pda) = setup();

    let result = build_touch_pair_no_alias(
        &provider,
        PROGRAM_ID,
        TouchPairNoAliasAccounts {
            a: vault_a_pda,
            b: vault_a_pda,
        },
    )
    .send_and_confirm();

    assert!(
        result.is_err(),
        "the same account passed into two unaliased mut slots must be rejected \
         (NaclacError::ConstraintDuplicateMutableAccount)"
    );
}

#[test]
fn duplicate_mutable_account_with_alias_is_accepted() {
    let (provider, vault_a_pda, _vault_b_pda) = setup();

    let result = build_touch_pair_with_alias(
        &provider,
        PROGRAM_ID,
        TouchPairWithAliasAccounts {
            a: vault_a_pda,
            b: vault_a_pda,
        },
    )
    .send_and_confirm();

    assert!(
        result.is_ok(),
        "unsafe(alias) on both fields should allow passing the same account twice: {:?}",
        result.err()
    );
}
