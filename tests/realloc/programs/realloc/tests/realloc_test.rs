use naclac_client::*;
use realloc_client::{
    get_growable_pda,
    instructions::{
        build_init_growable, build_resize_growable, InitGrowableAccounts, ResizeGrowableAccounts,
    },
    types::PROGRAM_ID,
};

fn load_program(provider: &NaclacProvider) {
    let mut so_path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    so_path.pop(); // programs
    so_path.pop(); // realloc workspace root
    so_path.push("target/deploy/realloc.so");

    provider
        .add_program(&PROGRAM_ID, so_path.to_str().unwrap())
        .expect("Failed to load realloc program binary");
}

fn setup() -> NaclacProvider {
    let payer = load_node_wallet().expect("Failed to load local Solana keypair");
    let provider = NaclacProvider::new("litesvm", payer);
    load_program(&provider);
    provider
}

/// Growing: the account's real on-chain byte length actually changes, the
/// payer actually funds the rent top-up, and — critically — the newly
/// allocated bytes are genuinely zeroed. This last part was flagged in
/// `TEST_PLAN.md` as a suspected dead-code gap (`realloc::zero` parsed but
/// never wired up); tracing the underlying `resize()` primitives directly
/// showed both backends already zero new memory unconditionally themselves,
/// but reasoning from code isn't the same as observing it — this is the
/// actual observation.
#[test]
fn growing_resizes_funds_and_zeroes_new_bytes() {
    let provider = setup();
    let (growable_pda, _bump) = get_growable_pda(&PROGRAM_ID);

    build_init_growable(
        &provider,
        PROGRAM_ID,
        InitGrowableAccounts {
            payer: provider.payer.address(),
            growable: growable_pda,
            system_program: Address::default(),
        },
    )
    .send_and_confirm()
    .expect("init_growable should succeed");

    let initial_account = provider.get_account(&growable_pda).expect("growable should exist");
    let initial_len = initial_account.data.len();
    let initial_lamports = initial_account.lamports;

    let payer_lamports_before = provider
        .get_account(&provider.payer.address())
        .expect("payer should exist")
        .lamports;

    let new_space: u64 = 500;
    build_resize_growable(
        &provider,
        PROGRAM_ID,
        new_space,
        ResizeGrowableAccounts {
            payer: provider.payer.address(),
            growable: growable_pda,
            system_program: Address::default(),
        },
    )
    .send_and_confirm()
    .expect("resize_growable (grow) should succeed");

    let grown_account = provider.get_account(&growable_pda).expect("growable should exist");
    assert_eq!(
        grown_account.data.len(),
        new_space as usize,
        "the account's real on-chain byte length must match the requested new space"
    );
    assert!(
        grown_account.lamports > initial_lamports,
        "growing must top up lamports to stay rent-exempt at the new size"
    );

    let payer_lamports_after = provider
        .get_account(&provider.payer.address())
        .expect("payer should exist")
        .lamports;
    assert!(
        payer_lamports_after < payer_lamports_before,
        "the rent top-up must actually come from the payer"
    );

    assert!(
        grown_account.data[initial_len..].iter().all(|&b| b == 0),
        "newly-grown bytes must be zeroed, not left as stale/uninitialized memory"
    );
}

/// Shrinking: the account's real byte length actually decreases, and the
/// now-excess rent is refunded back to the payer — the real bug found
/// while reading `realloc.rs` (no refund logic existed at all before this
/// fix) and fixed as part of this test case, not left as an unverified
/// code-reading claim.
#[test]
fn shrinking_resizes_and_refunds_excess_rent() {
    let provider = setup();
    let (growable_pda, _bump) = get_growable_pda(&PROGRAM_ID);

    build_init_growable(
        &provider,
        PROGRAM_ID,
        InitGrowableAccounts {
            payer: provider.payer.address(),
            growable: growable_pda,
            system_program: Address::default(),
        },
    )
    .send_and_confirm()
    .expect("init_growable should succeed");

    build_resize_growable(
        &provider,
        PROGRAM_ID,
        1000,
        ResizeGrowableAccounts {
            payer: provider.payer.address(),
            growable: growable_pda,
            system_program: Address::default(),
        },
    )
    .send_and_confirm()
    .expect("resize_growable (grow to 1000) should succeed");

    let grown_lamports = provider
        .get_account(&growable_pda)
        .expect("growable should exist")
        .lamports;
    let payer_lamports_before_shrink = provider
        .get_account(&provider.payer.address())
        .expect("payer should exist")
        .lamports;

    let smaller_space: u64 = 50;
    build_resize_growable(
        &provider,
        PROGRAM_ID,
        smaller_space,
        ResizeGrowableAccounts {
            payer: provider.payer.address(),
            growable: growable_pda,
            system_program: Address::default(),
        },
    )
    .send_and_confirm()
    .expect("resize_growable (shrink) should succeed");

    let shrunk_account = provider.get_account(&growable_pda).expect("growable should exist");
    assert_eq!(shrunk_account.data.len(), smaller_space as usize);
    assert!(
        shrunk_account.lamports < grown_lamports,
        "shrinking must reduce the account's lamports back toward the new, smaller \
         rent-exempt minimum instead of leaving it over-funded forever"
    );

    let payer_lamports_after_shrink = provider
        .get_account(&provider.payer.address())
        .expect("payer should exist")
        .lamports;
    assert!(
        payer_lamports_after_shrink > payer_lamports_before_shrink,
        "the excess rent freed by shrinking must actually be refunded to the payer"
    );
}
