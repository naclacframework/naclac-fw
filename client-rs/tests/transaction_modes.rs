use {
    naclac_client::{
        load_node_wallet, AccountMeta, InstructionBuilder, NaclacProvider, SignerAddressExt,
        TransactionVersion, VersionedTransaction, SYSTEM_PROGRAM_ID,
    },
    solana_address::Address,
    solana_keypair::Keypair,
    solana_message::VersionedMessage,
};

fn system_transfer_builder<'a>(
    provider: &'a NaclacProvider,
    to: Address,
    lamports: u64,
) -> InstructionBuilder<'a> {
    let mut data = vec![2, 0, 0, 0]; // SystemInstruction::Transfer discriminant
    data.extend_from_slice(&lamports.to_le_bytes());

    InstructionBuilder::new(provider, SYSTEM_PROGRAM_ID, data)
        .account(AccountMeta::new(provider.payer.address(), true), "from")
        .account(AccountMeta::new(to, false), "to")
}

#[test]
fn test_send_v1_transaction_default() {
    let payer = Keypair::new();
    let provider = NaclacProvider::new("litesvm", payer).expect("failed to construct provider");
    let recipient = Address::new_unique();

    let meta = system_transfer_builder(&provider, recipient, 10_000_000)
        .send_and_confirm()
        .expect("v1 default transaction must succeed");

    assert!(!meta.signature.to_string().is_empty());
}

#[test]
fn test_send_v1_transaction_with_config() {
    let payer = Keypair::new();
    let provider = NaclacProvider::new("litesvm", payer).expect("failed to construct provider");
    let recipient = Address::new_unique();

    let meta = system_transfer_builder(&provider, recipient, 10_000_000)
        .v1()
        .compute_unit_limit(150_000)
        .priority_fee(5_000)
        .send_and_confirm()
        .expect("v1 transaction with custom config must succeed");

    assert!(!meta.signature.to_string().is_empty());
}

#[test]
fn test_send_v0_transaction() {
    let payer = Keypair::new();
    let provider = NaclacProvider::new("litesvm", payer).expect("failed to construct provider");
    let recipient = Address::new_unique();

    let meta = system_transfer_builder(&provider, recipient, 10_000_000)
        .v0()
        .send_and_confirm()
        .expect("v0 transaction must succeed");

    assert!(!meta.signature.to_string().is_empty());
}

#[test]
fn test_send_legacy_transaction() {
    let payer = Keypair::new();
    let provider = NaclacProvider::new("litesvm", payer).expect("failed to construct provider");
    let recipient = Address::new_unique();

    let meta = system_transfer_builder(&provider, recipient, 10_000_000)
        .legacy()
        .send_and_confirm()
        .expect("legacy transaction must succeed");

    assert!(!meta.signature.to_string().is_empty());
}

#[test]
fn test_provider_default_switch() {
    let payer = Keypair::new();
    let provider = NaclacProvider::new("litesvm", payer)
        .expect("failed to construct provider")
        .with_tx_version(TransactionVersion::V0);

    let recipient = Address::new_unique();
    let builder = system_transfer_builder(&provider, recipient, 10_000_000);

    assert_eq!(builder.version, TransactionVersion::V0);

    let meta = builder
        .send_and_confirm()
        .expect("inherited v0 transaction must succeed");

    assert!(!meta.signature.to_string().is_empty());
}

#[test]
fn test_transaction_message_versions() {
    let payer = Keypair::new();
    let provider = NaclacProvider::new("litesvm", payer).expect("failed to construct provider");
    let recipient = Address::new_unique();

    let v1_tx = system_transfer_builder(&provider, recipient, 10_000_000)
        .v1()
        .transaction()
        .expect("must compile v1 transaction");
    assert!(matches!(v1_tx.message, VersionedMessage::V1(_)));

    let v0_tx = system_transfer_builder(&provider, recipient, 10_000_000)
        .v0()
        .transaction()
        .expect("must compile v0 transaction");
    assert!(matches!(v0_tx.message, VersionedMessage::V0(_)));

    let legacy_tx = system_transfer_builder(&provider, recipient, 10_000_000)
        .legacy()
        .transaction()
        .expect("must compile legacy transaction");
    assert!(matches!(legacy_tx.message, VersionedMessage::Legacy(_)));
}

#[test]
fn test_read_v1_transaction_metadata() {
    let payer = Keypair::new();
    let provider = NaclacProvider::new("litesvm", payer).expect("failed to construct provider");
    let recipient = Address::new_unique();

    let meta = system_transfer_builder(&provider, recipient, 10_000_000)
        .v1()
        .send_and_confirm()
        .expect("v1 transaction must succeed");

    let read_meta = provider
        .get_transaction(&meta.signature)
        .expect("must successfully read landed v1 transaction");

    assert_eq!(read_meta.signature, meta.signature);
    assert_eq!(read_meta.logs, meta.logs);
    assert_eq!(
        read_meta.compute_units_consumed,
        meta.compute_units_consumed
    );
}

#[test]
fn test_v1_compute_unit_limit_exceeded() {
    let payer = Keypair::new();
    let provider = NaclacProvider::new("litesvm", payer).expect("failed to construct provider");
    let recipient = Address::new_unique();

    let res = system_transfer_builder(&provider, recipient, 10_000_000)
        .v1()
        .compute_unit_limit(10)
        .send_and_confirm();

    assert!(res.is_err(), "expected computational budget exceeded");
}

#[test]
fn test_v1_priority_fee_deduction() {
    let payer = Keypair::new();
    let provider = NaclacProvider::new("litesvm", payer).expect("failed to construct provider");
    let recipient = Address::new_unique();
    let payer_addr = provider.payer.address();

    let initial_balance = provider.get_balance(&payer_addr).expect("get balance");
    let transfer_amount = 10_000_000;
    let priority_fee = 25_000;

    let meta = system_transfer_builder(&provider, recipient, transfer_amount)
        .v1()
        .priority_fee(priority_fee)
        .send_and_confirm()
        .expect("v1 transaction must succeed");

    assert!(!meta.signature.to_string().is_empty());

    let final_balance = provider.get_balance(&payer_addr).expect("get balance");
    assert_eq!(
        initial_balance - final_balance,
        5_000 + priority_fee + transfer_amount
    );
}

#[test]
fn test_simulate_v1_transaction() {
    let payer = Keypair::new();
    let provider = NaclacProvider::new("litesvm", payer).expect("failed to construct provider");
    let recipient = Address::new_unique();

    let meta = system_transfer_builder(&provider, recipient, 10_000_000)
        .v1()
        .simulate()
        .expect("v1 simulation must succeed");

    assert!(meta.compute_units_consumed > 0);
    assert!(!meta.logs.is_empty());

    let recipient_balance = provider.get_balance(&recipient).unwrap_or(0);
    assert_eq!(recipient_balance, 0);
}

#[test]
fn test_v1_wire_serialization_and_deserialization() {
    let payer = Keypair::new();
    let provider = NaclacProvider::new("litesvm", payer).expect("failed to construct provider");
    let recipient = Address::new_unique();

    let v1_tx = system_transfer_builder(&provider, recipient, 10_000_000)
        .v1()
        .compute_unit_limit(200_000)
        .priority_fee(1_000)
        .transaction()
        .expect("must build v1 transaction");

    let wire_bytes = bincode::serialize(&v1_tx).expect("serialize wire bytes");
    let deserialized: VersionedTransaction =
        bincode::deserialize(&wire_bytes).expect("deserialize wire bytes");

    assert!(matches!(deserialized.message, VersionedMessage::V1(_)));

    let meta = provider
        .send_transaction(&deserialized, None)
        .expect("sending deserialized v1 transaction must succeed");

    assert_eq!(meta.signature, v1_tx.signatures[0]);
}

#[test]
fn test_wire_prefix_versions() {
    let payer = Keypair::new();
    let provider = NaclacProvider::new("litesvm", payer).expect("failed to construct provider");
    let recipient = Address::new_unique();

    let v1_tx = system_transfer_builder(&provider, recipient, 10_000_000)
        .v1()
        .transaction()
        .expect("build v1");
    let v0_tx = system_transfer_builder(&provider, recipient, 10_000_000)
        .v0()
        .transaction()
        .expect("build v0");
    let legacy_tx = system_transfer_builder(&provider, recipient, 10_000_000)
        .legacy()
        .transaction()
        .expect("build legacy");

    let v1_msg_bytes = v1_tx.message.serialize();
    let v0_msg_bytes = v0_tx.message.serialize();
    let legacy_msg_bytes = legacy_tx.message.serialize();

    assert_eq!(v1_msg_bytes[0], 0x81);
    assert_eq!(v0_msg_bytes[0], 0x80);
    assert!(legacy_msg_bytes[0] < 0x80);
}

#[test]
fn test_utils_inherit_provider_tx_version() {
    let payer = Keypair::new();
    let provider = NaclacProvider::new("litesvm", payer).expect("provider");
    let recipient = Address::new_unique();

    let sig = naclac_client::transfer_sol(&provider, &recipient, 10_000_000).expect("transfer");
    let meta = provider.get_transaction(&sig).expect("get landed tx");
    assert_eq!(meta.signature, sig);

    let provider_v0 = provider.with_tx_version(TransactionVersion::V0);
    let recipient2 = Address::new_unique();
    let sig2 =
        naclac_client::transfer_sol(&provider_v0, &recipient2, 10_000_000).expect("transfer v0");
    let meta2 = provider_v0
        .get_transaction(&sig2)
        .expect("get landed tx v0");
    assert_eq!(meta2.signature, sig2);
}

fn setup_devnet_test() -> Result<(NaclacProvider, Address), &'static str> {
    let payer = load_node_wallet().map_err(|_| "missing ~/.config/solana/id.json")?;
    let provider =
        NaclacProvider::new("devnet", payer).map_err(|_| "failed to connect to devnet")?;
    let payer_addr = provider.payer.address();
    let balance = provider
        .get_balance(&payer_addr)
        .map_err(|_| "failed to fetch devnet balance")?;
    if balance < 10_000 {
        return Err("insufficient devnet balance (needs at least 10_000 lamports)");
    }
    Ok((provider, payer_addr))
}

#[test]
#[ignore = "requires network access and a funded devnet keypair at ~/.config/solana/id.json"]
fn test_devnet_send_v1_transaction_default() {
    let (provider, payer_addr) = setup_devnet_test().expect("devnet setup");
    let meta = system_transfer_builder(&provider, payer_addr, 1_000)
        .send_and_confirm()
        .expect("devnet v1 default transaction must succeed");

    println!("Devnet V1 Default Signature: {}", meta.signature);
    println!(
        "Explorer: https://explorer.solana.com/tx/{}?cluster=devnet",
        meta.signature
    );
    assert!(!meta.signature.to_string().is_empty());
}

#[test]
#[ignore = "requires network access and a funded devnet keypair at ~/.config/solana/id.json"]
fn test_devnet_send_v1_transaction_with_config() {
    let (provider, payer_addr) = setup_devnet_test().expect("devnet setup");
    let meta = system_transfer_builder(&provider, payer_addr, 1_000)
        .v1()
        .compute_unit_limit(150_000)
        .priority_fee(1_000)
        .send_and_confirm()
        .expect("devnet v1 custom config transaction must succeed");

    println!("Devnet V1 Custom Config Signature: {}", meta.signature);
    println!(
        "Explorer: https://explorer.solana.com/tx/{}?cluster=devnet",
        meta.signature
    );
    assert!(!meta.signature.to_string().is_empty());
}

#[test]
#[ignore = "requires network access and a funded devnet keypair at ~/.config/solana/id.json"]
fn test_devnet_send_v0_transaction() {
    let (provider, payer_addr) = setup_devnet_test().expect("devnet setup");
    let meta = system_transfer_builder(&provider, payer_addr, 1_000)
        .v0()
        .send_and_confirm()
        .expect("devnet v0 transaction must succeed");

    println!("Devnet V0 Signature: {}", meta.signature);
    println!(
        "Explorer: https://explorer.solana.com/tx/{}?cluster=devnet",
        meta.signature
    );
    assert!(!meta.signature.to_string().is_empty());
}

#[test]
#[ignore = "requires network access and a funded devnet keypair at ~/.config/solana/id.json"]
fn test_devnet_send_legacy_transaction() {
    let (provider, payer_addr) = setup_devnet_test().expect("devnet setup");
    let meta = system_transfer_builder(&provider, payer_addr, 1_000)
        .legacy()
        .send_and_confirm()
        .expect("devnet legacy transaction must succeed");

    println!("Devnet Legacy Signature: {}", meta.signature);
    println!(
        "Explorer: https://explorer.solana.com/tx/{}?cluster=devnet",
        meta.signature
    );
    assert!(!meta.signature.to_string().is_empty());
}

#[test]
#[ignore = "requires network access and a funded devnet keypair at ~/.config/solana/id.json"]
fn test_devnet_provider_default_switch() {
    let (provider, payer_addr) = setup_devnet_test().expect("devnet setup");
    let provider_v0 = provider.with_tx_version(TransactionVersion::V0);
    let builder = system_transfer_builder(&provider_v0, payer_addr, 1_000);
    assert_eq!(builder.version, TransactionVersion::V0);

    let meta = builder
        .send_and_confirm()
        .expect("devnet provider switched v0 transaction must succeed");

    println!("Devnet Switched V0 Signature: {}", meta.signature);
    println!(
        "Explorer: https://explorer.solana.com/tx/{}?cluster=devnet",
        meta.signature
    );
    assert!(!meta.signature.to_string().is_empty());
}

#[test]
#[ignore = "requires network access and a funded devnet keypair at ~/.config/solana/id.json"]
fn test_devnet_read_v1_transaction_metadata() {
    let (provider, payer_addr) = setup_devnet_test().expect("devnet setup");
    let meta = system_transfer_builder(&provider, payer_addr, 1_000)
        .v1()
        .send_and_confirm()
        .expect("devnet v1 transaction must succeed");

    let read_meta = provider
        .get_transaction(&meta.signature)
        .expect("devnet get_transaction must return landed v1 transaction");

    assert_eq!(read_meta.signature, meta.signature);
    assert_eq!(read_meta.logs, meta.logs);
    assert!(read_meta.compute_units_consumed > 0);
    println!(
        "Devnet Read V1 CU Consumed: {}",
        read_meta.compute_units_consumed
    );
}

#[test]
#[ignore = "requires network access and a funded devnet keypair at ~/.config/solana/id.json"]
fn test_devnet_simulate_v1_transaction() {
    let (provider, payer_addr) = setup_devnet_test().expect("devnet setup");
    let sim_meta = system_transfer_builder(&provider, payer_addr, 1_000)
        .v1()
        .simulate()
        .expect("devnet v1 simulation must succeed");

    assert!(sim_meta.compute_units_consumed > 0);
    assert!(!sim_meta.logs.is_empty());
    println!(
        "Devnet V1 Simulation CU: {}",
        sim_meta.compute_units_consumed
    );
}

#[test]
#[ignore = "requires network access and a funded devnet keypair at ~/.config/solana/id.json"]
fn test_devnet_v1_compute_unit_limit_exceeded() {
    let (provider, payer_addr) = setup_devnet_test().expect("devnet setup");
    let res = system_transfer_builder(&provider, payer_addr, 1_000)
        .v1()
        .compute_unit_limit(10)
        .simulate();

    assert!(
        res.is_err(),
        "expected computational budget exceeded on devnet"
    );
}

#[test]
#[ignore = "requires network access and a funded devnet keypair at ~/.config/solana/id.json"]
fn test_devnet_utils_inherit_provider_tx_version() {
    let (provider, payer_addr) = setup_devnet_test().expect("devnet setup");
    let sig = naclac_client::transfer_sol(&provider, &payer_addr, 1_000)
        .expect("devnet transfer_sol utility must succeed");

    println!("Devnet transfer_sol Signature: {}", sig);
    println!(
        "Explorer: https://explorer.solana.com/tx/{}?cluster=devnet",
        sig
    );
    let read_meta = provider
        .get_transaction(&sig)
        .expect("devnet get_transaction");
    assert_eq!(read_meta.signature, sig);
}
