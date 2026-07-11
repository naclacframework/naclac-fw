use naclac_client::*;
use discriminator_borsh_client::{
    instructions::{
        build_init_config, build_init_vault, build_read_vault, InitConfigAccounts,
        InitVaultAccounts, ReadVaultAccounts,
    },
    get_config_pda, get_vault_pda,
    types::PROGRAM_ID,
};

fn load_program(provider: &NaclacProvider) {
    let mut so_path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    so_path.pop(); // programs
    so_path.pop(); // discriminator workspace root
    so_path.push("target/deploy/discriminator_borsh.so");

    provider
        .add_program(&PROGRAM_ID, so_path.to_str().unwrap())
        .expect("Failed to load discriminator_borsh program binary");
}

/// The regression case for ZERO_COPY_BORSH_PARITY_AUDIT.md finding #1:
/// initialize a real `Config` account, then try to pass it into
/// `read_vault`'s `vault: Account<Vault>` slot. Both types are owned by the
/// same program, so the only thing that can reject this is the
/// discriminator check inside `Account::<Vault>::try_from`.
#[test]
fn rejects_type_confused_account_in_vault_slot() {
    let payer = load_node_wallet().expect("Failed to load local Solana keypair");
    let provider = NaclacProvider::new("litesvm", payer);
    load_program(&provider);

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

    let result = build_read_vault(
        &provider,
        PROGRAM_ID,
        ReadVaultAccounts {
            caller: provider.payer.address(),
            vault: config_pda,
        },
    )
    .send_and_confirm();

    assert!(
        result.is_err(),
        "read_vault must reject a Config account passed in the Vault slot — \
         if this assertion fails, the discriminator check has regressed \
         (see ZERO_COPY_BORSH_PARITY_AUDIT.md finding #1)"
    );
}

/// Positive-path companion: a correctly-typed Vault account must still be
/// accepted by the exact same instruction.
#[test]
fn accepts_correctly_typed_vault_account() {
    let payer = load_node_wallet().expect("Failed to load local Solana keypair");
    let provider = NaclacProvider::new("litesvm", payer);
    load_program(&provider);

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

    let result = build_read_vault(
        &provider,
        PROGRAM_ID,
        ReadVaultAccounts {
            caller: provider.payer.address(),
            vault: vault_pda,
        },
    )
    .send_and_confirm();

    assert!(
        result.is_ok(),
        "read_vault should succeed on a correctly-typed Vault account: {:?}",
        result.err()
    );
}
