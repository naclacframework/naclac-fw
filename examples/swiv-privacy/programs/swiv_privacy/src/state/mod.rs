//! Persistent zero-copy protocol, pool, and participant accounts.

pub mod bet;
pub mod pool;
pub mod protocol;

pub use {
    bet::{Bet, BetStatus},
    pool::{Pool, PoolStatus},
    protocol::Protocol,
};
