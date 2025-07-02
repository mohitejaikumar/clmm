use anchor_lang::prelude::*;

#[account]
#[derive(InitSpace)]
pub struct SupportMint {
    pub bump: u8,
    pub mint: Pubkey,
}
