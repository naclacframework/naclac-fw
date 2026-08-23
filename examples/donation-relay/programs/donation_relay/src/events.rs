use naclac_lang::prelude::*;

#[event(alloc)]
pub struct DonationMadeV1Event {
    pub config_id: Address,
    pub mint: Address,
    pub gross_amount: u64,
    pub tip: u64,
    pub message: String,
    pub credited_to: Address,
}
