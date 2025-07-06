use anchor_lang::prelude::*;

use crate::{
    helpers::{max_tick_in_tickarray_bitmap, MAX_TICK},
    state::{TickArrayBitmapExtension, TickArrayState},
};

// #[repr(C)] ensures a predictable, C-style memory layout for your struct
#[account(zero_copy)]
#[repr(C, packed)]
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

    pub sqrt_price_x64: u128, // sqrt(token_1/token_0) in Q64.64 format

    pub tick_current: i32, // current tick

    // Q64.64 format fee/liquidity for entire life of pool
    pub fee_growth_global_0_x64: u128,
    pub fee_growth_global_1_x64: u128,

    pub protocol_fees_token_0: u64,
    pub protocol_fees_token_1: u64,

    pub swap_in_amount_token_0: u128,
    pub swap_out_amount_token_1: u128,
    pub swap_in_amount_token_1: u128,
    pub swap_out_amount_token_0: u128,

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

impl PoolState {
    pub const LEN: usize =
        8 + 1 + 32 * 6 + 1 + 1 + 2 + 16 + 16 + 4 + 16 + 16 + 8 * 2 + 16 * 4 + 8 * 16 + 8 * 8;

    pub fn seeds(&self) -> [&[u8]; 5] {
        [
            &POOL_SEED.as_bytes(),
            self.amm_config.as_ref(),
            self.token_mint_0.as_ref(),
            self.token_mint_1.as_ref(),
            self.bump.as_ref(),
        ]
    }

    pub fn key(&self) -> Pubkey {
        Pubkey::create_program_address(&self.seeds(), &crate::id()).unwrap()
    }

    pub fn is_overflow_default_tickarray_bitmap(&self, tick_indexes: Vec<i32>) -> bool {
        let (min_tick_array_start_index_boundary, max_tick_array_index_boundary) =
            self.tick_array_start_index_range();

        for tick_index in tick_indexes {
            let tick_array_start_index =
                TickArrayState::get_array_start_index(tick_index, self.tick_spacing);
            if tick_array_start_index >= max_tick_array_index_boundary
                || tick_array_start_index < min_tick_array_start_index_boundary
            {
                return true;
            }
        }
        false
    }

    pub fn tick_array_start_index_range(&self) -> (i32, i32) {
        let mut max_tick_boundary = max_tick_in_tickarray_bitmap(self.tick_spacing);

        let mut min_tick_boundary = -max_tick_boundary;
        if max_tick_boundary > MAX_TICK {
            max_tick_boundary = TickArrayState::get_array_start_index(MAX_TICK, self.tick_spacing);
            // next tick array start index its will be exclusive upperbound
            max_tick_boundary = max_tick_boundary + TickArrayState::tick_count(self.tick_spacing);
        }
        if min_tick_boundary < MIN_TICK {
            min_tick_boundary = TickArrayState::get_array_start_index(MIN_TICK, self.tick_spacing);
        }
        (min_tick_boundary, max_tick_boundary)
    }

    pub fn flip_tick_array_bit<'c: 'info, 'info>(
        &mut self,
        tickarray_bitmap_extension: Option<&'c AccountInfo<'info>>,
        tick_array_start_index: i32,
    ) -> Result<()> {
        if self.is_overflow_default_tickarray_bitmap(vec![tick_array_start_index]) {
            require_keys_eq!(
                tickarray_bitmap_extension.unwrap().key(),
                TickArrayBitmapExtension::key(self.key())
            );
            AccountLoader::<TickArrayBitmapExtension>::try_from(
                tickarray_bitmap_extension.unwrap(),
            )?
            .load_mut()?
            .flip_tick_array_bit(tick_array_start_index, self.tick_spacing)
        } else {
            self.flip_tick_array_bit_internal(tick_array_start_index)
        }
    }
}
