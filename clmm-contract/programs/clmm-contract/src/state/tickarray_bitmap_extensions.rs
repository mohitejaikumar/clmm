use anchor_lang::prelude::*;

use crate::{
    helpers::{max_tick_in_tickarray_bitmap, MAX_TICK, MIN_TICK, U512},
    state::TickArrayState,
};

pub const TICK_ARRAY_BITMAP_SIZE: i32 = 512;

const EXTENSION_TICKARRAY_BITMAP_SIZE: usize = 14; // number of tick array bitmap
pub type TickArryBitmap = [u64; 8];

#[account(zero_copy)]
#[repr(C, packed)]
#[derive(InitSpace)]
pub struct TickArrayBitmapExtension {
    pub pool_id: Pubkey,
    pub positive_tick_array_bitmap: [[u64; 8]; EXTENSION_TICKARRAY_BITMAP_SIZE], // each bit is tick-array-index
    pub negative_tick_array_bitmap: [[u64; 8]; EXTENSION_TICKARRAY_BITMAP_SIZE], // each bit is tick-array-index
}

impl TickArrayBitmapExtension {
    pub fn initialize(&mut self, pool_id: Pubkey) {
        self.pool_id = pool_id;
        self.positive_tick_array_bitmap = [[0; 8]; EXTENSION_TICKARRAY_BITMAP_SIZE];
        self.negative_tick_array_bitmap = [[0; 8]; EXTENSION_TICKARRAY_BITMAP_SIZE];
    }

    pub fn key(pool_id: Pubkey) -> Pubkey {
        Pubkey::find_program_address(&[b"tick_array_bitmap", pool_id.as_ref()], &crate::id()).0
    }

    /// Flip the value of tick in the bitmap.
    pub fn flip_tick_array_bit(
        &mut self,
        tick_array_start_index: i32,
        tick_spacing: u16,
    ) -> Result<()> {
        let (offset, tick_array_bitmap) = self.get_bitmap(tick_array_start_index, tick_spacing)?;
        let tick_array_offset_in_bitmap =
            Self::tick_array_offset_in_bitmap(tick_array_start_index, tick_spacing);
        let tick_array_bitmap = U512(tick_array_bitmap);
        let mask = U512::one() << tick_array_offset_in_bitmap;
        if tick_array_start_index < 0 {
            self.negative_tick_array_bitmap[offset as usize] = tick_array_bitmap.bitxor(mask).0;
        } else {
            self.positive_tick_array_bitmap[offset as usize] = tick_array_bitmap.bitxor(mask).0;
        }
        Ok(())
    }

    fn get_bitmap_offset(tick_index: i32, tick_spacing: u16) -> Result<usize> {
        require!(
            TickArrayState::check_is_valid_start_index(tick_index, tick_spacing),
            ErrorCode::InvalidTickIndex
        );
        Self::check_extension_boundary(tick_index, tick_spacing)?;
        let ticks_in_one_bitmap = max_tick_in_tickarray_bitmap(tick_spacing);
        let mut offset = tick_index.abs() / ticks_in_one_bitmap - 1;
        if tick_index < 0 && tick_index.abs() % ticks_in_one_bitmap == 0 {
            offset -= 1;
        }
        Ok(offset as usize)
    }

    /// According to the given tick, calculate its corresponding tickarray and then find the bitmap it belongs to.
    fn get_bitmap(&self, tick_index: i32, tick_spacing: u16) -> Result<(usize, TickArryBitmap)> {
        let offset = Self::get_bitmap_offset(tick_index, tick_spacing)?;
        if tick_index < 0 {
            Ok((offset, self.negative_tick_array_bitmap[offset]))
        } else {
            Ok((offset, self.positive_tick_array_bitmap[offset]))
        }
    }

    /// Check if the tick in tick array bitmap extension
    pub fn check_extension_boundary(tick_index: i32, tick_spacing: u16) -> Result<()> {
        let positive_tick_boundary = max_tick_in_tickarray_bitmap(tick_spacing);
        let negative_tick_boundary = -positive_tick_boundary;
        require_gt!(MAX_TICK, positive_tick_boundary);
        require_gt!(negative_tick_boundary, MIN_TICK);
        if tick_index >= negative_tick_boundary && tick_index < positive_tick_boundary {
            return err!(ErrorCode::InvalidTickArrayBoundary);
        }
        Ok(())
    }

    /// Check if the tick array is initialized
    pub fn check_tick_array_is_initialized(
        &self,
        tick_array_start_index: i32,
        tick_spacing: u16,
    ) -> Result<(bool, i32)> {
        let (_, tickarray_bitmap) = self.get_bitmap(tick_array_start_index, tick_spacing)?;

        let tick_array_offset_in_bitmap =
            Self::tick_array_offset_in_bitmap(tick_array_start_index, tick_spacing);

        if U512(tickarray_bitmap).bit(tick_array_offset_in_bitmap as usize) {
            return Ok((true, tick_array_start_index));
        }
        Ok((false, tick_array_start_index))
    }

    // negative ticks from right to left
    pub fn tick_array_offset_in_bitmap(tick_array_start_index: i32, tick_spacing: u16) -> i32 {
        let m = tick_array_start_index.abs() % max_tick_in_tickarray_bitmap(tick_spacing);
        let mut tick_array_offset_in_bitmap = m / TickArrayState::tick_count(tick_spacing);
        if tick_array_start_index < 0 && m != 0 {
            tick_array_offset_in_bitmap = TICK_ARRAY_BITMAP_SIZE - tick_array_offset_in_bitmap;
        }
        tick_array_offset_in_bitmap
    }
}

#[error_code]
pub enum ErrorCode {
    #[msg("Invalid tick index")]
    InvalidTickIndex,
    #[msg("Invalid tick array boundary")]
    InvalidTickArrayBoundary,
}
