//! The account whose enum fields the tests exercise.

use crate::types::{Action, Color, Level, Limits, Signed, Wide};
use naclac_lang::prelude::*;

#[account]
pub struct State {
    pub bump: u8,
    pub admin: Address,
    pub color: ZcEnum<Color>,
    pub action: ZcEnum<Action>,
    pub history: [ZcEnum<Color>; 4],
    pub maybe_color: Opt<ZcEnum<Color>>,
    pub limits: Limits,
    pub level: ZcEnum<Level>,
    pub signed: ZcEnum<Signed>,
    pub wide: ZcEnum<Wide>,
    pub counter: u8,
}
