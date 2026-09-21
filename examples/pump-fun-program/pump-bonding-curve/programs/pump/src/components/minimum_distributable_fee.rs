use naclac_lang::prelude::*;

// Plain return-value type (not stored on-chain, not emitted as a log event)
// mirroring `pump_fees::components::Fees`'s own placement/shape — the real
// IDL names this type `MinimumDistributableFeeEvent` despite it only ever
// being returned, never emitted, same convention as `Fees`/`get_fees`.
#[defined_type]
pub struct MinimumDistributableFeeEvent {
    pub minimum_required: u64,
    pub distributable_fees: u64,
    pub can_distribute: Bool,
}
