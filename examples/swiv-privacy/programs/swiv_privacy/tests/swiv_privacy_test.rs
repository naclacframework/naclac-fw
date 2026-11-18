//! Deterministic LiteSVM regressions for the public L1 pool lifecycle.

use {
    naclac_client::{
        create_mint, create_token_account, mint_to, AccountMeta, Address, FixedString,
        InstructionBuilder, InstructionError, Keypair, NaclacClientError, NaclacError,
        NaclacProvider, NaclacTransactionMetadata, SignerAddressExt, SYSTEM_PROGRAM_ID,
        TOKEN_PROGRAM_ID,
    },
    std::path::Path,
    swiv_privacy_client::{
        instructions::*,
        state::{fetch_bet, fetch_pool, fetch_protocol, Bet, Pool},
        types::{BetStatus, OptAddress, OptI64, OptU64, PoolStatus, ProgramError, PROGRAM_ID},
    },
};

const START: i64 = 1_700_000_000;
const END: i64 = START + 200;
const USER_TOKENS: u64 = 10_000;
type TxResult = Result<NaclacTransactionMetadata, NaclacClientError>;

struct User {
    key: Keypair,
    tokens: Address,
}

#[derive(Clone, Copy)]
struct Market {
    pool: Address,
    vault: Address,
}

struct Fixture {
    provider: NaclacProvider,
    protocol: Address,
    mint: Address,
    treasury: Address,
}

impl Fixture {
    fn new(fee_bps: u64) -> Self {
        let binary = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join("..")
            .join("artifacts")
            .join("deploy")
            .join("swiv_privacy.so");
        assert!(
            binary.is_file(),
            "Missing SBF program {}. Build swiv_privacy with naclac build before running these tests.",
            binary.display()
        );
        let provider = NaclacProvider::new_litesvm(Keypair::new_from_array([1; 32])).unwrap();
        provider
            .add_program(&PROGRAM_ID, binary.to_str().unwrap())
            .unwrap();
        provider.set_clock_unix_timestamp(START).unwrap();
        let treasury_wallet = Keypair::new_from_array([2; 32]).address();
        provider.airdrop(&treasury_wallet, 1_000_000_000).unwrap();
        let mint = Keypair::new_from_array([3; 32]);
        create_mint(&provider, &mint, &provider.payer.address(), 0).unwrap();
        let treasury = Keypair::new_from_array([4; 32]);
        create_token_account(&provider, &treasury, &mint.address(), &treasury_wallet).unwrap();
        let protocol = Address::find_program_address(&[b"protocol_v1"], &PROGRAM_ID).0;
        let fixture = Self {
            provider,
            protocol,
            mint: mint.address(),
            treasury: treasury.address(),
        };
        fixture
            .send(build_initialize_protocol(
                &fixture.provider,
                PROGRAM_ID,
                fee_bps,
                InitializeProtocolAccounts {
                    protocol,
                    admin: fixture.admin(),
                    treasury_wallet,
                    system_program: SYSTEM_PROGRAM_ID,
                },
            ))
            .unwrap();
        let protocol = fetch_protocol(&fixture.provider, &protocol).unwrap();
        assert_eq!({ protocol.admin }, fixture.admin());
        assert_eq!({ protocol.treasury_wallet }, treasury_wallet);
        assert_eq!({ protocol.protocol_fee_bps }, fee_bps);
        assert_eq!({ protocol.batch_settle_wait_duration }, 60);
        fixture
    }

    fn admin(&self) -> Address {
        self.provider.payer.address()
    }

    fn send(&self, builder: InstructionBuilder<'_>) -> TxResult {
        self.provider.expire_blockhash().unwrap();
        builder.legacy().send_and_confirm()
    }

    fn clock(&self, timestamp: i64) {
        self.provider.set_clock_unix_timestamp(timestamp).unwrap();
    }

    fn user(&self, seed: u8) -> User {
        let key = Keypair::new_from_array([seed; 32]);
        self.provider
            .airdrop(&key.address(), 1_000_000_000)
            .unwrap();
        let tokens = Keypair::new_from_array([seed + 1; 32]);
        create_token_account(&self.provider, &tokens, &self.mint, &key.address()).unwrap();
        mint_to(
            &self.provider,
            &self.mint,
            &tokens.address(),
            &self.provider.payer,
            USER_TOKENS,
        )
        .unwrap();
        User {
            key,
            tokens: tokens.address(),
        }
    }

    fn market(&self) -> Market {
        let id = fetch_protocol(&self.provider, &self.protocol)
            .unwrap()
            .total_pools;
        let pool = Address::find_program_address(
            &[b"pool", self.admin().as_ref(), &id.to_le_bytes()],
            &PROGRAM_ID,
        )
        .0;
        let vault = Address::find_program_address(&[b"pool_vault", pool.as_ref()], &PROGRAM_ID).0;
        Market { pool, vault }
    }

    fn create_pool(&self, end: i64, conviction: u64) -> TxResult {
        let market = self.market();
        let mut title = FixedString::<128>::new();
        title.try_set(&"x".repeat(128)).unwrap();
        self.send(build_create_pool(
            &self.provider,
            PROGRAM_ID,
            title,
            START,
            end,
            10,
            conviction,
            CreatePoolAccounts {
                created_by: self.admin(),
                protocol: self.protocol,
                pool: market.pool,
                token_mint: self.mint,
                pool_vault: market.vault,
                token_program: TOKEN_PROGRAM_ID,
                system_program: SYSTEM_PROGRAM_ID,
            },
        ))
    }

    fn pool(&self, market: Market) -> Pool {
        fetch_pool(&self.provider, &market.pool).unwrap()
    }

    fn bet_address(&self, market: Market, user: &User) -> Address {
        Address::find_program_address(
            &[b"bet", market.pool.as_ref(), user.key.address().as_ref()],
            &PROGRAM_ID,
        )
        .0
    }

    fn bet(&self, market: Market, user: &User) -> Bet {
        fetch_bet(&self.provider, &self.bet_address(market, user)).unwrap()
    }

    fn balance(&self, tokens: Address) -> u64 {
        self.provider.get_token_balance(&tokens).unwrap().0
    }

    fn data(&self, address: Address) -> Vec<u8> {
        self.provider.get_account_data(&address).unwrap()
    }

    fn absent(&self, address: Address) {
        assert!(
            matches!(
                self.provider.get_account(&address),
                Err(NaclacClientError::AccountNotFound(_))
            ),
            "Account {address} must be closed or uncreated"
        );
    }

    fn init(&self, market: Market, user: &User, amount: u64) -> TxResult {
        self.send(
            build_init_bet(
                &self.provider,
                PROGRAM_ID,
                amount,
                InitBetAccounts {
                    user: user.key.address(),
                    sponsor: self.admin(),
                    protocol: self.protocol,
                    pool: market.pool,
                    token_mint: self.mint,
                    pool_vault: market.vault,
                    user_token_account: user.tokens,
                    bet: self.bet_address(market, user),
                    token_program: TOKEN_PROGRAM_ID,
                    system_program: SYSTEM_PROGRAM_ID,
                },
            )
            .signer(&user.key),
        )
    }

    fn place(&self, market: Market, user: &User, prediction: u64) -> TxResult {
        self.send(
            build_place_bet(
                &self.provider,
                PROGRAM_ID,
                prediction,
                PlaceBetAccounts {
                    user: user.key.address(),
                    protocol: self.protocol,
                    pool: market.pool,
                    bet: self.bet_address(market, user),
                },
            )
            .signer(&user.key),
        )
    }

    fn update(&self, market: Market, user: &User, prediction: u64, stake: u64) -> TxResult {
        self.send(
            build_update_bet(
                &self.provider,
                PROGRAM_ID,
                prediction,
                stake,
                UpdateBetAccounts {
                    user: user.key.address(),
                    protocol: self.protocol,
                    pool: market.pool,
                    bet: self.bet_address(market, user),
                    token_mint: self.mint,
                    pool_vault: market.vault,
                    user_token_account: user.tokens,
                    token_program: TOKEN_PROGRAM_ID,
                },
            )
            .signer(&user.key),
        )
    }

    fn add(&self, market: Market, user: &User, stake: u64) -> TxResult {
        self.send(
            build_add_stake(
                &self.provider,
                PROGRAM_ID,
                stake,
                AddStakeAccounts {
                    user: user.key.address(),
                    protocol: self.protocol,
                    pool: market.pool,
                    bet: self.bet_address(market, user),
                    token_mint: self.mint,
                    pool_vault: market.vault,
                    user_token_account: user.tokens,
                    token_program: TOKEN_PROGRAM_ID,
                },
            )
            .signer(&user.key),
        )
    }

    fn resolve(&self, market: Market, outcome: u64) -> TxResult {
        self.send(build_resolve_pool(
            &self.provider,
            PROGRAM_ID,
            outcome,
            ResolvePoolAccounts {
                admin: self.admin(),
                protocol: self.protocol,
                pool: market.pool,
            },
        ))
    }

    fn cancel(&self, market: Market) -> TxResult {
        self.send(build_cancel_pool(
            &self.provider,
            PROGRAM_ID,
            CancelPoolAccounts {
                admin: self.admin(),
                protocol: self.protocol,
                pool: market.pool,
            },
        ))
    }

    fn score(&self, market: Market, bets: &[Address]) -> TxResult {
        self.send(
            build_batch_calculate_weights(
                &self.provider,
                PROGRAM_ID,
                BatchCalculateWeightsAccounts {
                    admin: self.admin(),
                    pool: market.pool,
                },
            )
            .remaining_accounts(
                bets.iter()
                    .map(|bet| AccountMeta::new(*bet, false))
                    .collect(),
            ),
        )
    }

    fn finalize(&self, market: Market, treasury: Address) -> TxResult {
        self.send(build_finalize_weights(
            &self.provider,
            PROGRAM_ID,
            FinalizeWeightsAccounts {
                admin: self.admin(),
                protocol: self.protocol,
                pool: market.pool,
                token_mint: self.mint,
                pool_vault: market.vault,
                treasury_token_account: treasury,
                token_program: TOKEN_PROGRAM_ID,
            },
        ))
    }

    fn claim(&self, market: Market, user: &User, treasury: Address) -> TxResult {
        self.send(
            build_claim_reward(
                &self.provider,
                PROGRAM_ID,
                ClaimRewardAccounts {
                    user: user.key.address(),
                    sponsor: self.admin(),
                    protocol: self.protocol,
                    pool: market.pool,
                    bet: self.bet_address(market, user),
                    token_mint: self.mint,
                    pool_vault: market.vault,
                    user_token_account: user.tokens,
                    treasury_token_account: treasury,
                    token_program: TOKEN_PROGRAM_ID,
                },
            )
            .signer(&user.key),
        )
    }

    fn refund(&self, market: Market, user: &User) -> TxResult {
        self.send(
            build_emergency_refund(
                &self.provider,
                PROGRAM_ID,
                EmergencyRefundAccounts {
                    user: user.key.address(),
                    sponsor: self.admin(),
                    protocol: self.protocol,
                    pool: market.pool,
                    bet: self.bet_address(market, user),
                    token_mint: self.mint,
                    pool_vault: market.vault,
                    user_token_account: user.tokens,
                    treasury_token_account: self.treasury,
                    token_program: TOKEN_PROGRAM_ID,
                },
            )
            .signer(&user.key),
        )
    }
}

fn program_error(result: TxResult, expected: ProgramError) {
    match result.unwrap_err() {
        NaclacClientError::TransactionFailed {
            instruction_err: InstructionError::Custom(code),
            logs,
            ..
        } => {
            assert_eq!(
                ProgramError::from_code(code),
                Some(expected),
                "logs: {logs:?}"
            );
        }
        error => panic!("Expected on-chain {expected:?}, got {error:?}"),
    }
}

fn framework_error(result: TxResult, expected: NaclacError) {
    match result.unwrap_err() {
        NaclacClientError::TransactionFailed {
            instruction_err: InstructionError::Custom(code),
            logs,
            ..
        } => {
            assert_eq!(
                naclac_client::decode_custom_error(code).map(|(error, _)| error),
                Some(expected),
                "logs: {logs:?}"
            );
        }
        error => panic!("Expected on-chain {expected:?}, got {error:?}"),
    }
}

#[test]
fn deposits_scoring_and_zero_weight_last_claim_are_atomic_and_order_independent() {
    for winner_order in [[0, 1], [1, 0]] {
        let f = Fixture::new(500);
        let market = f.market();
        f.create_pool(END, 9_999).unwrap();
        let pool = f.pool(market);
        assert_eq!(pool.title.as_str().unwrap(), "x".repeat(128));
        assert_eq!(
            (pool.start_time, pool.end_time, pool.cutoff_time),
            (START, END, END - 10)
        );
        let users = [f.user(10), f.user(20), f.user(30)];
        let first = &users[0];
        let pool_before = f.data(market.pool);
        let failed = f.init(market, first, USER_TOKENS + 1).unwrap_err();
        assert!(
            matches!(
                failed,
                NaclacClientError::TransactionFailed {
                    instruction_err: InstructionError::Custom(1),
                    ..
                }
            ),
            "{failed:?}"
        );
        assert_eq!(f.data(market.pool), pool_before);
        assert_eq!(f.balance(first.tokens), USER_TOKENS);
        assert_eq!(f.balance(market.vault), 0);
        f.absent(f.bet_address(market, first));

        f.init(market, first, 100).unwrap();
        f.place(market, first, 99).unwrap();
        assert_eq!({ f.bet(market, first).update_count }, 1);
        f.add(market, first, 100).unwrap();
        assert_eq!(
            (
                f.bet(market, first).prediction,
                f.bet(market, first).update_count
            ),
            (99, 1)
        );
        f.update(market, first, 100, 101).unwrap();
        assert_eq!(
            (
                f.bet(market, first).stake,
                f.bet(market, first).prediction,
                f.bet(market, first).update_count
            ),
            (301, 100, 2)
        );
        let bet_before = f.data(f.bet_address(market, first));
        let pool_before = f.data(market.pool);
        let failed = f.update(market, first, 999, USER_TOKENS).unwrap_err();
        assert!(
            matches!(
                failed,
                NaclacClientError::TransactionFailed {
                    instruction_err: InstructionError::Custom(1),
                    ..
                }
            ),
            "{failed:?}"
        );
        assert_eq!(f.data(f.bet_address(market, first)), bet_before);
        assert_eq!(f.data(market.pool), pool_before);
        assert_eq!(f.balance(first.tokens), USER_TOKENS - 301);
        assert_eq!(f.balance(market.vault), 301);
        f.init(market, &users[1], 700).unwrap();
        f.place(market, &users[1], 100).unwrap();
        f.init(market, &users[2], 200).unwrap();
        f.place(market, &users[2], 110).unwrap();
        let pool = f.pool(market);
        assert_eq!(
            (
                pool.total_staked,
                pool.total_participants,
                pool.remaining_participants
            ),
            (1_201, 3, 3)
        );
        assert_eq!(f.balance(market.vault), 1_201);
        program_error(f.resolve(market, 100), ProgramError::DurationTooShort);
        f.clock(END - 10);
        program_error(f.place(market, first, 100), ProgramError::MarketClosed);
        program_error(f.add(market, first, 1), ProgramError::MarketClosed);
        assert_eq!(f.balance(market.vault), 1_201);
        f.clock(END);
        program_error(
            f.send(
                build_resolve_pool(
                    &f.provider,
                    PROGRAM_ID,
                    100,
                    ResolvePoolAccounts {
                        admin: first.key.address(),
                        protocol: f.protocol,
                        pool: market.pool,
                    },
                )
                .signer(&first.key),
            ),
            ProgramError::Unauthorized,
        );
        f.resolve(market, 100).unwrap();
        f.clock(END + 60);
        program_error(
            f.finalize(market, f.treasury),
            ProgramError::WeightsIncomplete,
        );
        program_error(
            f.claim(market, first, f.treasury),
            ProgramError::SettlementTooEarly,
        );
        let bets = users
            .iter()
            .map(|user| f.bet_address(market, user))
            .collect::<Vec<_>>();
        f.score(market, &bets[..1]).unwrap();
        program_error(f.resolve(market, 101), ProgramError::AlreadyResolved);
        assert_eq!(
            (
                f.pool(market).resolution_result,
                f.pool(market).weights_calculated_count
            ),
            (100, 1)
        );
        program_error(
            f.finalize(market, f.treasury),
            ProgramError::WeightsIncomplete,
        );
        f.score(market, &bets).unwrap();
        let pool_before = f.data(market.pool);
        f.score(market, &bets).unwrap();
        assert_eq!(f.data(market.pool), pool_before);
        assert_eq!(
            (
                f.pool(market).total_weight,
                f.pool(market).weights_calculated_count
            ),
            (2_502, 3)
        );
        for (user, weight) in users.iter().zip([752, 1_750, 0]) {
            let bet = f.bet(market, user);
            assert_eq!({ bet.calculated_weight }, weight);
            assert!(matches!(bet.status.get(), Ok(BetStatus::Resolved)));
            assert!(bet.is_weight_added.get());
        }
        framework_error(
            f.finalize(market, first.tokens),
            NaclacError::ConstraintTokenAuthority,
        );
        let wrong_mint = Keypair::new_from_array([60; 32]);
        create_mint(&f.provider, &wrong_mint, &f.admin(), 0).unwrap();
        let wrong_treasury = Keypair::new_from_array([61; 32]);
        let treasury_wallet = fetch_protocol(&f.provider, &f.protocol)
            .unwrap()
            .treasury_wallet;
        create_token_account(
            &f.provider,
            &wrong_treasury,
            &wrong_mint.address(),
            &treasury_wallet,
        )
        .unwrap();
        framework_error(
            f.finalize(market, wrong_treasury.address()),
            NaclacError::ConstraintTokenMint,
        );
        assert_eq!(f.balance(f.treasury), 0);
        assert_eq!(f.balance(market.vault), 1_201);
        f.finalize(market, f.treasury).unwrap();
        assert!(matches!(
            f.pool(market).status.get(),
            Ok(PoolStatus::Resolved)
        ));
        assert_eq!({ f.pool(market).distributable_amount }, 1_141);
        assert_eq!(f.balance(f.treasury), 60);
        mint_to(&f.provider, &f.mint, &market.vault, &f.provider.payer, 11).unwrap();
        framework_error(
            f.claim(market, first, users[1].tokens),
            NaclacError::ConstraintTokenAuthority,
        );
        framework_error(
            f.claim(market, first, wrong_treasury.address()),
            NaclacError::ConstraintTokenMint,
        );
        for index in winner_order {
            f.claim(market, &users[index], f.treasury).unwrap();
            f.absent(bets[index]);
        }
        assert_eq!(f.balance(users[0].tokens), USER_TOKENS - 301 + 342);
        assert_eq!(f.balance(users[1].tokens), USER_TOKENS - 700 + 798);
        assert_eq!(f.balance(market.vault), 12);
        assert_eq!({ f.pool(market).remaining_participants }, 1);
        f.claim(market, &users[2], f.treasury).unwrap();
        assert_eq!(f.balance(users[2].tokens), USER_TOKENS - 200);
        assert_eq!(f.balance(f.treasury), 72);
        for bet in bets {
            f.absent(bet);
        }
        f.absent(market.vault);
        f.absent(market.pool);
    }
}

#[test]
fn scoring_rejects_wrong_owner_wrong_pool_and_forged_pda_without_partial_progress() {
    let f = Fixture::new(0);
    let market = f.market();
    f.create_pool(END, 0).unwrap();
    let user = f.user(10);
    f.init(market, &user, 100).unwrap();
    let other = f.market();
    f.create_pool(END, 0).unwrap();
    f.init(other, &user, 100).unwrap();
    let bet = f.bet_address(market, &user);
    let other_bet = f.bet_address(other, &user);
    let forged = Keypair::new_from_array([80; 32]).address();
    let account = f.provider.get_account(&bet).unwrap();
    f.provider
        .set_account(&forged, account.data, &PROGRAM_ID, account.lamports)
        .unwrap();
    f.clock(END);
    f.resolve(market, 100).unwrap();
    let pool_before = f.data(market.pool);
    let bet_before = f.data(bet);
    for (invalid, expected) in [
        (user.tokens, ProgramError::InvalidBet),
        (other_bet, ProgramError::PoolMismatch),
        (forged, ProgramError::SeedMismatch),
    ] {
        program_error(f.score(market, &[bet, invalid]), expected);
        assert_eq!(f.data(market.pool), pool_before);
        assert_eq!(f.data(bet), bet_before);
    }
    let readonly = build_batch_calculate_weights(
        &f.provider,
        PROGRAM_ID,
        BatchCalculateWeightsAccounts {
            admin: f.admin(),
            pool: market.pool,
        },
    )
    .remaining_accounts(vec![AccountMeta::new_readonly(bet, false)]);
    program_error(f.send(readonly), ProgramError::InvalidBet);
    assert_eq!(f.data(market.pool), pool_before);
    f.score(market, &[bet]).unwrap();
    assert_eq!({ f.pool(market).weights_calculated_count }, 1);
}

#[test]
fn no_winner_pool_refunds_stake_and_sends_only_unsolicited_tokens_to_treasury() {
    let f = Fixture::new(1_000);
    let market = f.market();
    f.create_pool(END, 0).unwrap();
    let users = [f.user(10), f.user(20)];
    for (user, stake) in users.iter().zip([100, 201]) {
        f.init(market, user, stake).unwrap();
        f.place(market, user, 110).unwrap();
    }
    f.clock(END);
    f.resolve(market, 100).unwrap();
    let bets = users
        .iter()
        .map(|user| f.bet_address(market, user))
        .collect::<Vec<_>>();
    f.score(market, &bets).unwrap();
    assert_eq!({ f.pool(market).total_weight }, 0);
    program_error(
        f.finalize(market, f.treasury),
        ProgramError::SettlementTooEarly,
    );
    f.clock(END + 60);
    f.finalize(market, f.treasury).unwrap();
    assert_eq!(f.balance(f.treasury), 0);
    mint_to(&f.provider, &f.mint, &market.vault, &f.provider.payer, 9).unwrap();
    for user in users.iter().rev() {
        f.claim(market, user, f.treasury).unwrap();
        assert_eq!(f.balance(user.tokens), USER_TOKENS);
        f.absent(f.bet_address(market, user));
    }
    assert_eq!(f.balance(f.treasury), 9);
    f.absent(market.pool);
    f.absent(market.vault);
}

#[test]
fn cancelled_and_timed_out_pools_refund_and_close_without_accepting_new_stake() {
    for cancelled in [true, false] {
        let f = Fixture::new(500);
        let market = f.market();
        f.create_pool(END, 0).unwrap();
        let users = [f.user(10), f.user(20), f.user(30)];
        f.init(market, &users[0], 100).unwrap();
        f.init(market, &users[1], 200).unwrap();
        program_error(f.refund(market, &users[0]), ProgramError::TimeoutNotMet);
        if cancelled {
            f.cancel(market).unwrap();
            let before = f.data(market.pool);
            program_error(f.init(market, &users[2], 50), ProgramError::MarketClosed);
            program_error(f.place(market, &users[0], 100), ProgramError::MarketClosed);
            program_error(
                f.update(market, &users[0], 100, 10),
                ProgramError::MarketClosed,
            );
            program_error(f.add(market, &users[0], 10), ProgramError::MarketClosed);
            assert_eq!(f.data(market.pool), before);
            assert_eq!(f.balance(market.vault), 300);
            f.absent(f.bet_address(market, &users[2]));
        } else {
            f.clock(END + 60);
            program_error(f.refund(market, &users[0]), ProgramError::TimeoutNotMet);
            f.clock(END + 61);
        }
        mint_to(&f.provider, &f.mint, &market.vault, &f.provider.payer, 7).unwrap();
        f.refund(market, &users[0]).unwrap();
        assert_eq!(
            (
                f.pool(market).total_staked,
                f.pool(market).total_participants,
                f.pool(market).remaining_participants
            ),
            (200, 1, 1)
        );
        f.refund(market, &users[1]).unwrap();
        for user in &users[..2] {
            assert_eq!(f.balance(user.tokens), USER_TOKENS);
            f.absent(f.bet_address(market, user));
        }
        assert_eq!(f.balance(f.treasury), 7);
        f.absent(market.pool);
        f.absent(market.vault);
    }
}

#[test]
fn invalid_times_and_optional_tags_fail_without_changing_protocol_or_creating_pool() {
    let f = Fixture::new(0);
    let market = f.market();
    let before = f.data(f.protocol);
    program_error(f.create_pool(START + 10, 0), ProgramError::InvalidTime);
    program_error(f.create_pool(START - 1, 0), ProgramError::DurationTooShort);
    assert_eq!(f.data(f.protocol), before);
    f.absent(market.pool);
    f.absent(market.vault);
    for index in 0..3 {
        let treasury = OptAddress {
            tag: if index == 0 { 2 } else { 0 },
            value: Address::default(),
        };
        let fee = OptU64 {
            tag: if index == 1 { 2 } else { 0 },
            value: 0,
        };
        let wait = OptI64 {
            tag: if index == 2 { 2 } else { 0 },
            value: 0,
        };
        program_error(
            f.send(build_update_config(
                &f.provider,
                PROGRAM_ID,
                treasury,
                fee,
                wait,
                UpdateConfigAccounts {
                    admin: f.admin(),
                    protocol: f.protocol,
                    system_program: SYSTEM_PROGRAM_ID,
                },
            )),
            ProgramError::InvalidOptionalValue,
        );
        assert_eq!(f.data(f.protocol), before);
    }
    program_error(
        f.send(build_update_config(
            &f.provider,
            PROGRAM_ID,
            OptAddress {
                tag: 0,
                value: Address::default(),
            },
            OptU64 { tag: 0, value: 0 },
            OptI64 { tag: 1, value: -1 },
            UpdateConfigAccounts {
                admin: f.admin(),
                protocol: f.protocol,
                system_program: SYSTEM_PROGRAM_ID,
            },
        )),
        ProgramError::InvalidTime,
    );
    assert_eq!(f.data(f.protocol), before);
}
