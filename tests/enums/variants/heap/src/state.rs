//! The account whose enum fields the tests exercise.

use crate::types::{Action, Color, Level, Limits, Wide};
use naclac_lang::prelude::*;

#[account]
pub struct State {
    pub bump: u8,
    pub admin: Address,
    pub color: Color,
    pub action: Action,
    pub history: [Color; 4],
    pub maybe_color: Option<Color>,
    pub limits: Limits,
    pub level: Level,
    pub wide: Wide,
    pub counter: u8,
}
