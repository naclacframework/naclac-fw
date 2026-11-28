use {crate::constants::MAX_MESSAGE_LEN, naclac_lang::prelude::*};

#[event]
pub struct DonationMadeV1Event {
    pub config_id: Address,
    pub mint: Address,
    pub gross_amount: u64,
    pub tip: u64,
    pub message: FixedString<MAX_MESSAGE_LEN>,
    pub credited_to: Address,
}
