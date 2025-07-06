use anchor_lang::prelude::*;
use uint::construct_uint;
// to reduce compute budget
construct_uint! {
    pub struct U128(2);
}

pub const MIN_TICK: i32 = -443636;
pub const MAX_TICK: i32 = -MIN_TICK;

pub const MIN_SQRT_PRICE_X64: u128 = 4295048016;
pub const MAX_SQRT_PRICE_X64: u128 = 79226673521066979257578248091;

pub const TICK_ARRAY_SIZE: i32 = 60;

const NUM_64: U128 = U128([64, 0]);

// formula: `i = long base(sqrt(1.0001) (sqrt(price))`
pub fn get_tick_at_sqrt_price(sqrt_price_x64: u128) -> Result<i32, Error> {
    require!(
        sqrt_price_x64 >= MIN_SQRT_PRICE_X64 && sqrt_price_x64 < MAX_SQRT_PRICE_X64,
        ErrorCode::InvalidSqrtPrice,
    );

    // calculate log2(sqrt_price_x64), msb of x is integral part of log2(x)
    let msb = 128 - sqrt_price_x64.leading_zeros() - 1; // this is 0based msb
    let log2p_integer_x32 = (msb as i128 - 64) << 32; // we will have 32 bits for fractional part

    /*
       let `j` is integral part of log2(x)
       let `f` is fractional part of log2(x)

       f = log2(x) - j;
       f = log2(x) - log2(2^j)
       f = log(x/2^j)

       now f = log2(y)
       for calulation we do approximation and its iterative
       0.5 in binary is 0.1   before `.` there are 64 bits and after `.` there are 64 bits
       0.25 in binary is 0.01
       0.125 in binary is 0.001

       so always question log2(y) > 0.5
       2log2(y) > 1
       log2(y^2) > 1
       y^2 > 2
       if yes then we will add that decimal to the answer

    */
    // r -> Q1.63
    // try to normalize r to get r in range of [1.0,2.0] in Q1.63
    let mut r = if msb >= 64 {
        sqrt_price_x64 >> (msb - 63)
    } else {
        sqrt_price_x64 << (63 - msb)
    };
    // start from 0.5 in Q64.64
    let mut bit: i128 = 0x8000_0000_0000_0000i128;
    let mut precision = 0;
    let mut log2p_fraction_x64 = 0;

    while bit > 0 && precision < 16 {
        r *= r;
        let is_r_more_than_two = r >> 127 as u32;
        r >>= 63 + is_r_more_than_two;
        log2p_fraction_x64 += bit * is_r_more_than_two as i128;
        bit >>= 1;
        precision += 1;
    }

    let log2p_fraction_x32 = log2p_fraction_x64 >> 32;
    let log2p_x32 = log2p_integer_x32 + log2p_fraction_x32;
    // multiplyed by  2/ log2(1.0001) in Q32.32
    let log_sqrt_10001_x64 = log2p_x32 * 59543866431248i128;
    // floor operation , 0.01 least-count of price model
    let tick_low = ((log_sqrt_10001_x64 - 184467440737095516i128) >> 64) as i32;
    // tick + 2 *(2^-precision) / log2(0.0001) + 0.01
    let tick_high = ((log_sqrt_10001_x64 + 15793534762490258745i128) >> 64) as i32;

    if tick_low == tick_high {
        Ok(tick_low)
    } else if get_sqrt_price_at_tick(tick_high).unwrap() <= sqrt_price_x64 {
        Ok(tick_high)
    } else {
        Ok(tick_low)
    }
}

pub fn get_sqrt_price_at_tick(tick: i32) -> Result<u128, Error> {
    let abs_tick = tick.abs() as u32;
    require!(abs_tick <= MAX_TICK as u32, ErrorCode::TickUpperOverflow);

    // i = 0
    let mut ratio = if abs_tick & 0x1 != 0 {
        U128([0xfffcb933bd6fb800, 0])
    } else {
        // 2^64
        U128([0, 1])
    };
    // i = 1
    if abs_tick & 0x2 != 0 {
        ratio = (ratio * U128([0xfff97272373d4000, 0])) >> NUM_64
    };
    // i = 2
    if abs_tick & 0x4 != 0 {
        ratio = (ratio * U128([0xfff2e50f5f657000, 0])) >> NUM_64
    };
    // i = 3
    if abs_tick & 0x8 != 0 {
        ratio = (ratio * U128([0xffe5caca7e10f000, 0])) >> NUM_64
    };
    // i = 4
    if abs_tick & 0x10 != 0 {
        ratio = (ratio * U128([0xffcb9843d60f7000, 0])) >> NUM_64
    };
    // i = 5
    if abs_tick & 0x20 != 0 {
        ratio = (ratio * U128([0xff973b41fa98e800, 0])) >> NUM_64
    };
    // i = 6
    if abs_tick & 0x40 != 0 {
        ratio = (ratio * U128([0xff2ea16466c9b000, 0])) >> NUM_64
    };
    // i = 7
    if abs_tick & 0x80 != 0 {
        ratio = (ratio * U128([0xfe5dee046a9a3800, 0])) >> NUM_64
    };
    // i = 8
    if abs_tick & 0x100 != 0 {
        ratio = (ratio * U128([0xfcbe86c7900bb000, 0])) >> NUM_64
    };
    // i = 9
    if abs_tick & 0x200 != 0 {
        ratio = (ratio * U128([0xf987a7253ac65800, 0])) >> NUM_64
    };
    // i = 10
    if abs_tick & 0x400 != 0 {
        ratio = (ratio * U128([0xf3392b0822bb6000, 0])) >> NUM_64
    };
    // i = 11
    if abs_tick & 0x800 != 0 {
        ratio = (ratio * U128([0xe7159475a2caf000, 0])) >> NUM_64
    };
    // i = 12
    if abs_tick & 0x1000 != 0 {
        ratio = (ratio * U128([0xd097f3bdfd2f2000, 0])) >> NUM_64
    };
    // i = 13
    if abs_tick & 0x2000 != 0 {
        ratio = (ratio * U128([0xa9f746462d9f8000, 0])) >> NUM_64
    };
    // i = 14
    if abs_tick & 0x4000 != 0 {
        ratio = (ratio * U128([0x70d869a156f31c00, 0])) >> NUM_64
    };
    // i = 15
    if abs_tick & 0x8000 != 0 {
        ratio = (ratio * U128([0x31be135f97ed3200, 0])) >> NUM_64
    };
    // i = 16
    if abs_tick & 0x10000 != 0 {
        ratio = (ratio * U128([0x9aa508b5b85a500, 0])) >> NUM_64
    };
    // i = 17
    if abs_tick & 0x20000 != 0 {
        ratio = (ratio * U128([0x5d6af8dedc582c, 0])) >> NUM_64
    };
    // i = 18
    if abs_tick & 0x40000 != 0 {
        ratio = (ratio * U128([0x2216e584f5fa, 0])) >> NUM_64
    }

    // Divide to obtain 1.0001^(2^(i - 1)) * 2^32 in numerator
    if tick > 0 {
        ratio = U128::MAX / ratio;
    }

    Ok(ratio.as_u128())
}

pub fn get_array_start_index(tick_index: i32, tick_spacing: u16) -> i32 {
    let ticks_in_array = TICK_ARRAY_SIZE * i32::from(tick_spacing);
    let mut start = tick_index / ticks_in_array;
    if tick_index < 0 && tick_index % ticks_in_array != 0 {
        start = start - 1;
        // for negative division rust round toward 0
    }
    start * ticks_in_array
}

pub fn check_tick_array_start_index(
    tick_array_start_index: i32,
    tick_index: i32,
    tick_spacing: u16,
) -> Result<()> {
    require!(tick_index >= MIN_TICK, ErrorCode::TickLowerOverflow);
    require!(tick_index <= MAX_TICK, ErrorCode::TickUpperOverflow);
    require_eq!(tick_index % tick_spacing as i32, 0, ErrorCode::InvalidTick);
    let correct_start_index = get_array_start_index(tick_index, tick_spacing);
    require_eq!(
        tick_array_start_index,
        correct_start_index,
        ErrorCode::InvalidTickArrayStartIndex
    );

    Ok(())
}

#[error_code]
enum ErrorCode {
    #[msg("Invalid sqrt price")]
    InvalidSqrtPrice,
    #[msg("Tick lower overflow")]
    TickLowerOverflow,
    #[msg("Tick upper overflow")]
    TickUpperOverflow,
    #[msg("Invalid tick")]
    InvalidTick,
    #[msg("Invalid tick array start index")]
    InvalidTickArrayStartIndex,
}
