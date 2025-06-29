use anchor_lang::prelude::*;



// #[repr(C)] ensures a predictable, C-style memory layout for your struct
#[account(zero_copy)]
#[repr(C, packed)]
#[derive(InitSpace)]
pub struct PoolState {

    pub bump: [u8; 1],
    pub amm_config: Pubkey,
    pub owner: Pubkey, // pool creator

    pub token_mint_0: Pubkey,
    pub token_mint_1: Pubkey,

    pub token_vault_0: Pubkey,
    pub token_vault_1: Pubkey,

    pub mint_decimals_0: u8,
    pub mint_decimals_1: u8,

    pub tick_spacing: u16,
    pub liquidity: u128,

    pub sqrt_price_x64: u128,  // sqrt(token_1/token_0) in Q64.64 format

    pub tick_current: i32, // current tick
    
    // Q64.64 format fee/liquidity for entire life of pool
    pub fee_growth_global_0_x64: u128,
    pub fee_growth_global_1_x64: u128,

    pub protocol_fees_token_0: u64,
    pub protocol_fees_token_1: u64,

    pub swap_in_amount_token_0: u64,
    pub swap_in_amount_token_1: u64,
    pub swap_out_amount_token_0: u64,
    pub swap_out_amount_token_1: u64,

    pub tick_array_bitmap: [u64; 16],
    
    // except protocol fee and fund fee
    pub total_fees_token_0: u64,
    pub total_fees_claimed_token_0: u64,
    pub total_fees_token_1: u64,
    pub total_fees_claimed_token_1: u64,


    pub fund_fees_token_0: u64,
    pub fund_fees_token_1: u64,

    pub open_time: u64,
    pub recent_epoch: u64,
}