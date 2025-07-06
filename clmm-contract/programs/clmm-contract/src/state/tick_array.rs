use anchor_lang::prelude::*;

use crate::{
    helpers::{create_or_allocate_account, TICK_ARRAY_SIZE},
    state::PoolState,
    util::get_recent_epoch,
};

pub const TICK_ARRAY_SIZE_USIZE: usize = 60;
pub const TICK_ARRAY_SIZE: i32 = TICK_ARRAY_SIZE_USIZE as i32;
pub const MIN_TICK: i32 = -443636;
pub const MAX_TICK: i32 = -MIN_TICK;

#[zero_copy(unsafe)]
#[repr(C, packed)]
#[derive(Default, Debug)]
pub struct TickState {
    pub tick: i32,
    pub liquidity_net: i128, // net liquidity added when tick is crossed from left to right
    pub liquidity_gross: u128, // total position liquidity that references this tick
    /// Fee growth per unit of liquidity on the _other_ side of this tick (relative to the current tick)
    /// only has relative meaning, not absolute — the value depends on when the tick is initialized
    pub fee_growth_outside_0_x64: u128,
    pub fee_growth_outside_1_x64: u128,
}

impl TickState {
    pub const LEN: usize = 4 + 16 + 16 + 16 + 16;

    pub fn check_is_out_of_bounds(tick: i32) -> bool {
        tick < MIN_TICK || tick > MAX_TICK
    }

    pub fn update(
        &mut self,
        tick_current: i32,
        liquidity_delta: i128,
        fee_growth_global_0_x64: u128,
        fee_growth_global_1_x64: u128,
        upper: bool,
    ) -> Result<bool> {
        let liquidity_gross_before = self.liquidity_gross;
        let liquidity_gross_after = if liquidity_delta > 0 {
            liquidity_gross_before + liquidity_delta as u128
        } else {
            liquidity_gross_before - u128::try_from(-liquidity_delta).unwrap()
        };

        let flipped = (liquidity_gross_after == 0) != (liquidity_gross_before == 0);
        if liquidity_gross_before == 0 {
            // by convention, we assume that all growth before a tick was initialized happened _below_ the tick
            if self.tick <= tick_current {
                self.fee_growth_outside_0_x64 = fee_growth_global_0_x64;
                self.fee_growth_outside_1_x64 = fee_growth_global_1_x64;
            }
        }

        self.liquidity_gross = liquidity_gross_after;

        // Difference array technique
        self.liquidity_net = if upper {
            self.liquidity_net.checked_sub(liquidity_delta)
        } else {
            self.liquidity_net.checked_add(liquidity_delta)
        }
        .unwrap();

        Ok(flipped)
    }

    pub fn check_is_out_of_boundary(tick: i32) -> bool {
        tick < MIN_TICK || tick > MAX_TICK
    }
}

#[account(zero_copy(unsafe))]
#[repr(C, packed)]
pub struct TickArrayState {
    pub pool_id: Pubkey,
    pub start_tick_index: i32,
    pub ticks: [TickState; TICK_ARRAY_SIZE_USIZE],
    pub initialized_tick_count: u8,
    pub recent_epoch: u64,
}

impl TickArrayState {
    pub const LEN: usize = 8 + 32 + 4 + TICK_ARRAY_SIZE_USIZE * TickState::LEN + 1 + 8;

    pub fn initialize(
        &mut self,
        start_index: i32,
        tick_spacing: u16,
        pool_key: Pubkey,
    ) -> Result<()> {
        TickArrayState::check_is_valid_start_index(start_index, tick_spacing);
        self.start_tick_index = start_index;
        self.pool_id = pool_key;
        self.recent_epoch = get_recent_epoch()?;
        Ok(())
    }

    pub fn get_or_create_tick_array<'info>(
        payer: AccountInfo<'info>,
        tick_array_account_info: AccountInfo<'info>,
        tick_array_start_index: i32,
        tick_spacing: u16,
        pool_state_loader: &AccountLoader<'info, PoolState>,
        system_program: AccountInfo<'info>,
    ) -> Result<AccountLoader<'info, TickArrayState>> {
        require!(
            TickArrayState::check_is_valid_start_index(tick_array_start_index, tick_spacing),
            ErrorCode::InvalidTickArrayStartIndex
        );

        let tick_array_state = if tick_array_account_info.owner == &system_program::ID {
            let (expect_pda_address, bump) = Pubkey::find_program_address(
                &[
                    b"tick_array",
                    pool_state_loader.key().as_ref(),
                    &tick_array_start_index.to_be_bytes(),
                ],
                &crate::id(),
            );

            require_keys_eq!(expect_pda_address, tick_array_account_info.key());

            create_or_allocate_account(
                &crate::id(),
                payer,
                system_program,
                tick_array_account_info.clone(),
                &[
                    b"tick_array",
                    pool_state_loader.key().as_ref(),
                    &tick_array_start_index.to_be_bytes(),
                    &[bump],
                ],
                TickArrayState::LEN,
            )?;
            let tick_array_state_loader = AccountLoader::<TickArrayState>::try_from_unchecked(
                &crate::id(),
                &tick_array_account_info,
            )?;
            {
                let mut tick_array_account = tick_array_state_loader.load_init()?;
                tick_array_account.initialize(
                    tick_array_start_index,
                    tick_spacing,
                    pool_state_loader.key(),
                )?;
            }
            tick_array_state_loader
        } else {
            AccountLoader::<TickArrayState>::try_from(&tick_array_account_info)?;
        };
        Ok(tick_array_state)
    }

    pub fn get_array_start_index(tick_index: i32, tick_spacing: u16) -> i32 {
        let ticks_in_array = TickArrayState::tick_count(tick_spacing);
        let mut start = tick_index / ticks_in_array;
        if tick_index < 0 && tick_index % ticks_in_array != 0 {
            start = start - 1;
            // for negative division rust round toward 0
        }
        start * ticks_in_array
    }

    pub fn check_is_valid_start_index(tick_index: i32, tick_spacing: u16) -> bool {
        if TickState::check_is_out_of_bounds(tick) {
            if tick_index > MAX_TICK {
                return false;
            }
            let min_start_index = TickArrayState::get_array_start_index(MIN_TICK, tick_spacing);
            return tick_index == min_start_index;
        }
        tick_index % TickArrayState::tick_count(tick_spacing) == 0
    }

    pub fn tick_count(tick_spacing: u16) -> i32 {
        TICK_ARRAY_SIZE * i32::from(tick_spacing)
    }

    pub fn get_tick_offset_in_array(self, tick_index: i32, tick_spacing: u16) -> Result<usize> {
        let start_tick_index = TickArrayState::get_array_start_index(tick_index, tick_spacing);
        require_eq!(
            start_tick_index,
            self.start_tick_index,
            ErrorCode::InvalidTickArray
        );

        let offset_in_array =
            ((tick_index - self.start_tick_index) / i32::from(tick_spacing)) as usize;

        Ok(offset_in_array)
    }

    pub fn get_tick_state_mut(
        &mut self,
        tick_index: i32,
        tick_spacing: u16,
    ) -> Result<&mut TickState> {
        let offset_in_array = self.get_tick_offset_in_array(tick_index, tick_spacing)?;
        Ok(&mut self.ticks[offset_in_array])
    }

    pub fn update_tick_state(
        &mut self,
        tick_index: i32,
        tick_spacing: u16,
        tick_state: TickState,
    ) -> Result<()> {
        let offset_in_array = self.get_tick_offset_in_array(tick_index, tick_spacing)?;
        self.ticks[offset_in_array] = tick_state;
        self.recent_epoch = get_recent_epoch()?;
        Ok(())
    }

    pub fn update_initialized_tick_count(&mut self, add: bool) -> Result<()> {
        if add {
            self.initialized_tick_count += 1;
        } else {
            self.initialized_tick_count -= 1;
        }
        Ok(())
    }

    pub fn check_is_valid_start_index(tick_index: i32, tick_spacing: u16) -> bool {
        if TickState::check_is_out_of_boundary(tick_index) {
            if tick_index > MAX_TICK {
                return false;
            }
            let min_start_index = TickArrayState::get_array_start_index(MIN_TICK, tick_spacing);
            return tick_index == min_start_index;
        }
        tick_index % TickArrayState::tick_count(tick_spacing) == 0
    }
}

// Calculates the fee growths inside of tick_lower and tick_upper based on their positions relative to tick_current.
/// `fee_growth_inside = fee_growth_global - fee_growth_below(lower) - fee_growth_above(upper)`

pub fn get_fee_growth_inside(
    tick_lower: &TickState,
    tick_upper: &TickState,
    tick_current: i32,
    fee_growth_global_0_x64: u128,
    fee_growth_global_1_x64: u128,
) -> (u128, u128) {
    // calculate fee growth below
    let (fee_growth_below_0_x64, fee_growth_below_1_x64) = if tick_current >= tick_lower.tick {
        (
            tick_lower.fee_growth_outside_0_x64,
            tick_lower.fee_growth_outside_1_x64,
        )
    } else {
        (
            fee_growth_global_0_x64
                .checked_sub(tick_lower.fee_growth_outside_0_x64)
                .unwrap(),
            fee_growth_global_1_x64
                .checked_sub(tick_lower.fee_growth_outside_1_x64)
                .unwrap(),
        )
    };

    // Calculate fee growth above
    let (fee_growth_above_0_x64, fee_growth_above_1_x64) = if tick_current < tick_upper.tick {
        (
            tick_upper.fee_growth_outside_0_x64,
            tick_upper.fee_growth_outside_1_x64,
        )
    } else {
        (
            fee_growth_global_0_x64
                .checked_sub(tick_upper.fee_growth_outside_0_x64)
                .unwrap(),
            fee_growth_global_1_x64
                .checked_sub(tick_upper.fee_growth_outside_1_x64)
                .unwrap(),
        )
    };
    let fee_growth_inside_0_x64 = fee_growth_global_0_x64
        .wrapping_sub(fee_growth_below_0_x64)
        .wrapping_sub(fee_growth_above_0_x64);
    let fee_growth_inside_1_x64 = fee_growth_global_1_x64
        .wrapping_sub(fee_growth_below_1_x64)
        .wrapping_sub(fee_growth_above_1_x64);

    (fee_growth_inside_0_x64, fee_growth_inside_1_x64)
}

#[error_code]
pub enum ErrorCode {
    #[msg("Invalid tick array start index")]
    InvalidTickArrayStartIndex,
    #[msg("Invalid tick array")]
    InvalidTickArray,
}
