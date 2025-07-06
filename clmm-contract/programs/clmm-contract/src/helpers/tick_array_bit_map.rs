use anchor_lang::prelude::*;
use crate::state::TICK_ARRAY_SIZE;

pub const TICK_ARRAY_BITMAP_SIZE: i32 = 8*64;


pub fn max_tick_in_tickarray_bitmap(tick_spacing: u16) -> i32 {
    i32::from(tick_spacing) * TICK_ARRAY_SIZE * TICK_ARRAY_BITMAP_SIZE
}

pub fn get_bitmap_tick_boundary(tick_array_start_index: i32, tick_spacing: u16) -> (i32, i32) {
    let ticks_in_one_bitmap: i32 = max_tick_in_tickarray_bitmap(tick_spacing);
    let mut m = tick_array_start_index.abs() / ticks_in_one_bitmap;
    if tick_array_start_index < 0 && tick_array_start_index.abs() % ticks_in_one_bitmap != 0 {
        m += 1;
    }
    let min_value: i32 = ticks_in_one_bitmap * m;
    if tick_array_start_index < 0 { 
        (-min_value, -min_value + ticks_in_one_bitmap)
    } else {
        (min_value, min_value + ticks_in_one_bitmap)
    }
}