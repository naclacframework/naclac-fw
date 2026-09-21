// ===========================================================================
// plugin_type.rs — raw `PluginType` discriminants, both backends
// ===========================================================================

//! Raw `PluginType` discriminants (Borsh unit-enum order, verified against
//! `mpl-core`'s real `generated::types::PluginType`), shared by both
//! backends — neither depends on the real `mpl-core` crate (see `asset.rs`'s
//! header), so this is the only representation either one has.

pub const ROYALTIES: u8 = 0;
pub const FREEZE_DELEGATE: u8 = 1;
pub const BURN_DELEGATE: u8 = 2;
pub const TRANSFER_DELEGATE: u8 = 3;
pub const UPDATE_DELEGATE: u8 = 4;
pub const PERMANENT_FREEZE_DELEGATE: u8 = 5;
pub const ATTRIBUTES: u8 = 6;
pub const PERMANENT_TRANSFER_DELEGATE: u8 = 7;
pub const PERMANENT_BURN_DELEGATE: u8 = 8;
pub const EDITION: u8 = 9;
pub const MASTER_EDITION: u8 = 10;
pub const ADD_BLOCKER: u8 = 11;
pub const IMMUTABLE_METADATA: u8 = 12;
pub const VERIFIED_CREATORS: u8 = 13;
pub const AUTOGRAPH: u8 = 14;
pub const BUBBLEGUM_V2: u8 = 15;
pub const FREEZE_EXECUTE: u8 = 16;
pub const PERMANENT_FREEZE_EXECUTE: u8 = 17;
pub const GROUPS: u8 = 18;
