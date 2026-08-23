use naclac_lang::prelude::*;

/// Scoped reimplementation of the real `pump_amm::GlobalConfig` — this pass
/// only needs `disable_flags` (bit 0 = `create_pool` disabled, checked by
/// `create_pool`), `admin` (checked by `toggle_boost`/`set_boost_authority`),
/// and `boost_authority`/`boost_enabled` (checked by `boost_buy_and_burn`/
/// `init_boost`), not the real account's other fee/whitelist fields.
#[component]
pub struct GlobalConfig {
    pub bump: u8,
    pub disable_flags: u8,
    pub boost_enabled: Bool,
    pub admin: Address,
    pub boost_authority: Address,
}
