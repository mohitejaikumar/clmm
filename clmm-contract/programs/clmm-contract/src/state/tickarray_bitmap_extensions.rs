use anchor_lang::prelude::*;

const EXTENSION_TICKARRAY_BITMAP_SIZE: usize = 14;

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
}
