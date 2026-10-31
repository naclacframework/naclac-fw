use naclac_lang::prelude::*;

#[account]
pub struct Note {
    pub bump: u8,
    #[max_len(200)]
    pub name: String,
}
