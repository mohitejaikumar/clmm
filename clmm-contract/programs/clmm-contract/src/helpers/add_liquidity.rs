use std::cell::RefMut;

use anchor_lang::prelude::*;
use anchor_spl::{token::Token, token_2022::Token2022, token_interface::Mint};

use crate::{
    helpers::{
        fixed_point_64, get_sqrt_price_at_tick, get_transfer_fee, transfer_from_user_to_pool_vault,
        U256,
    },
    state::{PoolState, ProtocolPositionState, TickArrayState, TickState},
};

pub fn add_liquidity<'b, 'info>(
    payer: &'b Signer<'info>,
    token_account_0: &'b AccountInfo<'info>,
    token_account_1: &'b AccountInfo<'info>,
    token_vault_0: &'b AccountInfo<'info>,
    token_vault_1: &'b AccountInfo<'info>,
    tick_array_lower_loader: &'b AccountLoader<'info, TickArrayState>,
    tick_array_upper_loader: &'b AccountLoader<'info, TickArrayState>,
    protocol_position: &mut ProtocolPositionState,
    token_program_2022: Option<&Program<'info, Token2022>>,
    token_program: &'b Program<'info, Token>,
    vault_0_mint: Option<Box<InterfaceAccount<'info, Mint>>>,
    vault_1_mint: Option<Box<InterfaceAccount<'info, Mint>>>,
    tick_array_bitmap_extension: Option<&'b AccountInfo<'info>>,
    pool_state: &mut RefMut<PoolState>,
    liquidity: &mut u128,
    amount_0_max: u64,
    amount_1_max: u64,
    tick_lower_index: i32,
    tick_upper_index: i32,
    base_flag: Option<bool>,
) -> Result<(u64, u64, u64, u64)> {
    if *liquidity == 0 {
        if base_flag.is_none() {
            return Ok((0, 0, 0, 0));
        }
        if base_flag.unwrap() {
            let amount_0_transfer_fee =
                get_transfer_fee(vault_0_mint.clone().unwrap(), amount_0_max).unwrap();
            *liquidity = get_liquidity_from_single_amount_0(
                pool_state.sqrt_price_x64,
                get_sqrt_price_at_tick(tick_lower_index)?,
                get_sqrt_price_at_tick(tick_upper_index)?,
                amount_0_max.checked_sub(amount_0_transfer_fee).unwrap(),
            );
            msg!(
                "liquidity: {}, amount_0_max:{}, amount_0_transfer_fee:{}",
                *liquidity,
                amount_0_max,
                amount_0_transfer_fee
            );
        } else {
            let amount_1_transfer_fee =
                get_transfer_fee(vault_1_mint.clone().unwrap(), amount_1_max).unwrap();
            *liquidity = liquidity_math::get_liquidity_from_single_amount_1(
                pool_state.sqrt_price_x64,
                tick_math::get_sqrt_price_at_tick(tick_lower_index)?,
                tick_math::get_sqrt_price_at_tick(tick_upper_index)?,
                amount_1_max.checked_sub(amount_1_transfer_fee).unwrap(),
            );
            msg!(
                "liquidity: {}, amount_1_max:{}, amount_1_transfer_fee:{}",
                *liquidity,
                amount_1_max,
                amount_1_transfer_fee
            );
        }
    }

    assert!(*liquidity > 0);
    let liquidity_before = pool_state.liquidity;
    require_keys_eq!(tick_array_lower_loader.load()?.pool_id, pool_state.key());
    require_keys_eq!(tick_array_upper_loader.load()?.pool_id, pool_state.key());

    // get tick_state
    let mut tick_lower_state = *tick_array_lower_loader
        .load_mut()?
        .get_tick_state_mut(tick_lower_index, pool_state.tick_spacing)?;
    let mut tick_upper_state = *tick_array_upper_loader
        .load_mut()?
        .get_tick_state_mut(tick_upper_index, pool_state.tick_spacing)?;

    if tick_lower_state.tick == 0 {
        tick_lower_state.tick = tick_lower_index;
    }
    if tick_upper_state.tick == 0 {
        tick_upper_state.tick = tick_upper_index;
    }

    let clock = Clock::get()?;

    let (amount_0, amount_1, flip_tick_lower, flip_tick_upper) = modify_position(
        i128::try_from(*liquidity).unwrap(),
        pool_state,
        protocol_position,
        &mut tick_lower_state,
        &mut tick_upper_state,
        clock.unix_timestamp as u64,
    )?;

    // update tick_state
    tick_array_lower_loader.load_mut()?.update_tick_state(
        tick_lower_index,
        pool_state.tick_spacing,
        tick_lower_state,
    )?;
    tick_array_upper_loader.load_mut()?.update_tick_state(
        tick_upper_index,
        pool_state.tick_spacing,
        tick_upper_state,
    )?;

    if flip_tick_lower {
        let mut tick_array_lower = tick_array_lower_loader.load_mut()?;
        let before_init_tick_count = tick_array_lower.initialized_tick_count;
        tick_array_lower.update_initialized_tick_count(true)?;

        if before_init_tick_count == 0 {
            pool_state.flip_tick_array_bit(
                tick_array_bitmap_extension,
                tick_array_lower.start_tick_index,
            )?;
        }
    }

    if flip_tick_upper {
        let mut tick_array_upper = tick_array_upper_loader.load_mut()?;
        let before_init_tick_count = tick_array_upper.initialized_tick_count;
        tick_array_upper.update_initialized_tick_count(true)?;

        if before_init_tick_count == 0 {
            pool_state.flip_tick_array_bit(
                tick_array_bitmap_extension,
                tick_array_upper.start_tick_index,
            )?;
        }
    }

    require!(
        amount_0 > 0 || amount_1 > 0,
        ErrorCode::ForbidBothZeroForSupplyLiquidity
    );

    let mut amount_0_transfer_fee = 0;
    let mut amount_1_transfer_fee = 0;

    if vault_0_mint.is_some() {
        amount_0_transfer_fee =
            get_transfer_inverse_fee(vault_0_mint.clone().unwrap(), amount_0).unwrap();
    }

    if vault_1_mint.is_some() {
        amount_1_transfer_fee =
            get_transfer_inverse_fee(vault_1_mint.clone().unwrap(), amount_1).unwrap();
    }

    msg!(
        "amount_0: {}, amount_0_transfer_fee: {}, amount_1: {}, amount_1_transfer_fee: {}",
        amount_0,
        amount_0_transfer_fee,
        amount_1,
        amount_1_transfer_fee
    );

    require_gte!(
        amount_1_max,
        amount_1 + amount_1_transfer_fee,
        ErrorCode::PriceSlippageCheck
    );

    require_gte!(
        amount_0_max,
        amount_0 + amount_0_transfer_fee,
        ErrorCode::PriceSlippageCheck
    );

    let mut token_2022_program_opt: Option<AccountInfo> = None;

    if token_program_2022.is_some() {
        token_2022_program_opt = Some(token_program_2022.clone().unwrap().to_account_info());
    }

    transfer_from_user_to_pool_vault(
        payer,
        token_account_0,
        token_vault_0,
        vault_0_mint,
        &token_program,
        token_2022_program_opt.clone(),
        amount_0 + amount_0_transfer_fee,
    )?;
    transfer_from_user_to_pool_vault(
        payer,
        token_account_1,
        token_vault_1,
        vault_1_mint,
        &token_program,
        token_2022_program_opt.clone(),
        amount_1 + amount_1_transfer_fee,
    )?;

    Ok((
        amount_0,
        amount_1,
        amount_0_transfer_fee,
        amount_1_transfer_fee,
    ))
}

pub fn modify_position(
    liquidity_delta: i128,
    pool_state: &mut RefMut<PoolState>,
    protocol_position_state: &mut ProtocolPositionState,
    tick_lower_state: &mut TickState,
    tick_upper_state: &mut TickState,
    timestamp: u64,
) -> Result<(u64, u64, bool, bool)> {
    let (flip_tick_lower, flip_tick_upper) = update_position(
        liquidity_delta,
        pool_state,
        protocol_position_state,
        tick_lower_state,
        tick_upper_state,
        timestamp,
    )?;
    let mut amount_0 = 0;
    let mut amount_1 = 0;

    if liquidity_delta != 0 {
        (amount_0, amount_1) = get_delta_amounts_signed(
            pool_state.tick_current,
            pool_state.sqrt_price_x64,
            tick_lower_state.tick,
            tick_upper_state.tick,
            liquidity_delta,
        )?;
        if pool_state.tick_current >= tick_lower_state.tick
            && pool_state.tick_current < tick_upper_state.tick
        {
            pool_state.liquidity = if liquidity_delta > 0 {
                pool_state.liquidity + u128::try_from(liquidity_delta).unwrap()
            } else {
                pool_state.liquidity - u128::try_from(-liquidity_delta).unwrap()
            };
        }
    }

    Ok((amount_0, amount_1, flip_tick_lower, flip_tick_upper))
}

/// Update the position with the given liquidity delta and tick
pub fn update_position(
    liquidity_delta: i128,
    pool_state: &mut RefMut<PoolState>,
    protocol_position_state: &mut ProtocolPositionState,
    tick_lower_state: &mut TickState,
    tick_upper_state: &mut TickState,
    timestamp: u64,
) -> Result<(bool, bool)> {
    // update the liquidity_net, fees growth outside 0/1 , calculate fee growth inside 0/1

    let mut flipped_lower = false;
    let mut flipped_upper = false;

    // update the ticks if liquidity delta is non-zero
    if liquidity_delta != 0 {
        // Update tick state and find if tick is flipped
        flipped_lower = tick_lower_state.update(
            pool_state.tick_current,
            liquidity_delta,
            pool_state.fee_growth_global_0_x64,
            pool_state.fee_growth_global_1_x64,
            false,
        )?;
        flipped_upper = tick_upper_state.update(
            pool_state.tick_current,
            liquidity_delta,
            pool_state.fee_growth_global_0_x64,
            pool_state.fee_growth_global_1_x64,
            true,
        )?;

        msg!(
            "tick_upper.reward_growths_outside_x64:{:?}, tick_lower.reward_growths_outside_x64:{:?}",
            identity(tick_upper_state.reward_growths_outside_x64),
            identity(tick_lower_state.reward_growths_outside_x64)
        );
    }

    // Update fees
    let (fee_growth_inside_0_x64, fee_growth_inside_1_x64) = tick_array::get_fee_growth_inside(
        tick_lower_state.deref(),
        tick_upper_state.deref(),
        pool_state.tick_current,
        pool_state.fee_growth_global_0_x64,
        pool_state.fee_growth_global_1_x64,
    );

    protocol_position_state.update(
        tick_lower_state.tick,
        tick_upper_state.tick,
        liquidity_delta,
        fee_growth_inside_0_x64,
        fee_growth_inside_1_x64,
    )?;

    if liquidity_delta < 0 {
        if flipped_lower {
            tick_lower_state.clear();
        }
        if flipped_upper {
            tick_upper_state.clear();
        }
    }

    Ok((flipped_lower, flipped_upper))
}

/// Computes the maximum amount of liquidity received for a given amount of token_0, token_1, the current
/// pool prices and the prices at the tick boundaries
pub fn get_liquidity_from_single_amount_0(
    sqrt_ratio_x64: u128,
    mut sqrt_ratio_a_x64: u128,
    mut sqrt_ratio_b_x64: u128,
    amount_0: u64,
) -> u128 {
    // sqrt_ratio_a_x64 should hold the smaller value
    if sqrt_ratio_a_x64 > sqrt_ratio_b_x64 {
        std::mem::swap(&mut sqrt_ratio_a_x64, &mut sqrt_ratio_b_x64);
    };

    if sqrt_ratio_x64 <= sqrt_ratio_a_x64 {
        // If P ≤ P_lower, only token_0 liquidity is active
        get_liquidity_from_amount_0(sqrt_ratio_a_x64, sqrt_ratio_b_x64, amount_0)
    } else if sqrt_ratio_x64 < sqrt_ratio_b_x64 {
        // If P_lower < P < P_upper, active liquidity is the minimum of the liquidity provided
        // by token_0 and token_1
        get_liquidity_from_amount_0(sqrt_ratio_x64, sqrt_ratio_b_x64, amount_0)
    } else {
        // If P ≥ P_upper, only token_1 liquidity is active
        0
    }
}

/// Computes the maximum amount of liquidity received for a given amount of token_0, token_1, the current
/// pool prices and the prices at the tick boundaries
pub fn get_liquidity_from_single_amount_1(
    sqrt_ratio_x64: u128,
    mut sqrt_ratio_a_x64: u128,
    mut sqrt_ratio_b_x64: u128,
    amount_1: u64,
) -> u128 {
    // sqrt_ratio_a_x64 should hold the smaller value
    if sqrt_ratio_a_x64 > sqrt_ratio_b_x64 {
        std::mem::swap(&mut sqrt_ratio_a_x64, &mut sqrt_ratio_b_x64);
    };

    if sqrt_ratio_x64 <= sqrt_ratio_a_x64 {
        // If P ≤ P_lower, only token_0 liquidity is active
        0
    } else if sqrt_ratio_x64 < sqrt_ratio_b_x64 {
        // If P_lower < P < P_upper, active liquidity is the minimum of the liquidity provided
        // by token_0 and token_1
        get_liquidity_from_amount_1(sqrt_ratio_a_x64, sqrt_ratio_x64, amount_1)
    } else {
        // If P ≥ P_upper, only token_1 liquidity is active
        get_liquidity_from_amount_1(sqrt_ratio_a_x64, sqrt_ratio_b_x64, amount_1)
    }
}

/// Computes the amount of liquidity received for a given amount of token_0 and price range
/// Calculates ΔL = Δx (√P_upper x √P_lower)/(√P_upper - √P_lower)
pub fn get_liquidity_from_amount_0(
    mut sqrt_ratio_a_x64: u128,
    mut sqrt_ratio_b_x64: u128,
    amount_0: u64,
) -> u128 {
    // sqrt_ratio_a_x64 should hold the smaller value
    if sqrt_ratio_a_x64 > sqrt_ratio_b_x64 {
        std::mem::swap(&mut sqrt_ratio_a_x64, &mut sqrt_ratio_b_x64);
    };
    let intermediate = U128::from(sqrt_ratio_a_x64)
        .mul_div_floor(
            U128::from(sqrt_ratio_b_x64),
            U128::from(fixed_point_64::Q64),
        )
        .unwrap();

    U128::from(amount_0)
        .mul_div_floor(
            intermediate,
            U128::from(sqrt_ratio_b_x64 - sqrt_ratio_a_x64),
        )
        .unwrap()
        .as_u128()
}

/// Computes the amount of liquidity received for a given amount of token_1 and price range
/// Calculates ΔL = Δy / (√P_upper - √P_lower)
pub fn get_liquidity_from_amount_1(
    mut sqrt_ratio_a_x64: u128,
    mut sqrt_ratio_b_x64: u128,
    amount_1: u64,
) -> u128 {
    // sqrt_ratio_a_x64 should hold the smaller value
    if sqrt_ratio_a_x64 > sqrt_ratio_b_x64 {
        std::mem::swap(&mut sqrt_ratio_a_x64, &mut sqrt_ratio_b_x64);
    };

    U128::from(amount_1)
        .mul_div_floor(
            U128::from(fixed_point_64::Q64),
            U128::from(sqrt_ratio_b_x64 - sqrt_ratio_a_x64),
        )
        .unwrap()
        .as_u128()
}

// gets the delta amount_0 for given liquidity and price range
/// # Formula
///
/// * `Δx = L * (1 / √P_lower - 1 / √P_upper)`
/// * i.e. `L * (√P_upper - √P_lower) / (√P_upper * √P_lower)`
pub fn get_delta_amount_0_unsigned(
    mut sqrt_ratio_a_x64: u128,
    mut sqrt_ratio_b_x64: u128,
    liquidity: u128,
    round_up: bool,
) -> Result<u64> {
    // sqrt_ratio_a_x64 should hold the smaller value
    if sqrt_ratio_a_x64 > sqrt_ratio_b_x64 {
        std::mem::swap(&mut sqrt_ratio_a_x64, &mut sqrt_ratio_b_x64);
    };

    let numerator_1 = U256::from(liquidity) << fixed_point_64::RESOLUTION;
    let numerator_2 = U256::from(sqrt_ratio_b_x64 - sqrt_ratio_a_x64);

    assert!(sqrt_ratio_a_x64 > 0);

    let result = if round_up {
        U256::div_rounding_up(
            numerator_1
                .mul_div_ceil(numerator_2, U256::from(sqrt_ratio_b_x64))
                .unwrap(),
            U256::from(sqrt_ratio_a_x64),
        )
    } else {
        numerator_1
            .mul_div_floor(numerator_2, U256::from(sqrt_ratio_b_x64))
            .unwrap()
            / U256::from(sqrt_ratio_a_x64)
    };
    if result > U256::from(u64::MAX) {
        return Err(ErrorCode::MaxTokenOverflow.into());
    }
    return Ok(result.as_u64());
}

/// Gets the delta amount_1 for given liquidity and price range
/// * `Δy = L (√P_upper - √P_lower)`
pub fn get_delta_amount_1_unsigned(
    mut sqrt_ratio_a_x64: u128,
    mut sqrt_ratio_b_x64: u128,
    liquidity: u128,
    round_up: bool,
) -> Result<u64> {
    // sqrt_ratio_a_x64 should hold the smaller value
    if sqrt_ratio_a_x64 > sqrt_ratio_b_x64 {
        std::mem::swap(&mut sqrt_ratio_a_x64, &mut sqrt_ratio_b_x64);
    };

    let result = if round_up {
        U256::from(liquidity).mul_div_ceil(
            U256::from(sqrt_ratio_b_x64 - sqrt_ratio_a_x64),
            U256::from(fixed_point_64::Q64),
        )
    } else {
        U256::from(liquidity).mul_div_floor(
            U256::from(sqrt_ratio_b_x64 - sqrt_ratio_a_x64),
            U256::from(fixed_point_64::Q64),
        )
    }
    .unwrap();
    if result > U256::from(u64::MAX) {
        return Err(ErrorCode::MaxTokenOverflow.into());
    }
    return Ok(result.as_u64());
}

/// Helper function to get signed delta amount_0 for given liquidity and price range
pub fn get_delta_amount_0_signed(
    sqrt_ratio_a_x64: u128,
    sqrt_ratio_b_x64: u128,
    liquidity: i128,
) -> Result<u64> {
    if liquidity < 0 {
        get_delta_amount_0_unsigned(
            sqrt_ratio_a_x64,
            sqrt_ratio_b_x64,
            u128::try_from(-liquidity).unwrap(),
            false,
        )
    } else {
        get_delta_amount_0_unsigned(
            sqrt_ratio_a_x64,
            sqrt_ratio_b_x64,
            u128::try_from(liquidity).unwrap(),
            true,
        )
    }
}

/// Helper function to get signed delta amount_1 for given liquidity and price range
pub fn get_delta_amount_1_signed(
    sqrt_ratio_a_x64: u128,
    sqrt_ratio_b_x64: u128,
    liquidity: i128,
) -> Result<u64> {
    if liquidity < 0 {
        get_delta_amount_1_unsigned(
            sqrt_ratio_a_x64,
            sqrt_ratio_b_x64,
            u128::try_from(-liquidity).unwrap(),
            false,
        )
    } else {
        get_delta_amount_1_unsigned(
            sqrt_ratio_a_x64,
            sqrt_ratio_b_x64,
            u128::try_from(liquidity).unwrap(),
            true,
        )
    }
}

pub fn get_delta_amounts_signed(
    tick_current: i32,
    sqrt_price_x64_current: u128,
    tick_lower: i32,
    tick_upper: i32,
    liquidity_delta: i128,
) -> Result<(u64, u64)> {
    let mut amount_0 = 0;
    let mut amount_1 = 0;
    if tick_current < tick_lower {
        amount_0 = get_delta_amount_0_signed(
            get_sqrt_price_at_tick(tick_lower)?,
            get_sqrt_price_at_tick(tick_upper)?,
            liquidity_delta,
        )
        .unwrap();
    } else if tick_current < tick_upper {
        amount_0 = get_delta_amount_0_signed(
            sqrt_price_x64_current,
            get_sqrt_price_at_tick(tick_upper)?,
            liquidity_delta,
        )
        .unwrap();
        amount_1 = get_delta_amount_1_signed(
            get_sqrt_price_at_tick(tick_lower)?,
            sqrt_price_x64_current,
            liquidity_delta,
        )
        .unwrap();
    } else {
        amount_1 = get_delta_amount_1_signed(
            get_sqrt_price_at_tick(tick_lower)?,
            get_sqrt_price_at_tick(tick_upper)?,
            liquidity_delta,
        )
        .unwrap();
    }
    Ok((amount_0, amount_1))
}

#[error_code]
pub enum ErrorCode {
    #[msg("Max token overflow")]
    MaxTokenOverflow,
    #[msg("Invalid amount")]
    ForbidBothZeroForSupplyLiquidity,
    #[msg("Price slippage check")]
    PriceSlippageCheck,
}
