use anchor_lang::prelude::*;

#[account]
#[derive(InitSpace)]
pub struct AmmConfig {
    pub bump: u8,
    pub index: u16,
    // protocol owner
    pub owner: Pubkey,
    // protocol fees
    pub protocol_fee_rate: u32,
    pub trade_fee_rate: u32,
    pub fund_fee_rate: u32,

    pub tick_spacing: u16,
    pub fund_owner: Pubkey,
}
