use anchor_lang::prelude::*;

use crate::{
    helpers::{MAX_TICK, MIN_TICK},
    util::get_recent_epoch,
};

#[account]
#[derive(InitSpace)]
pub struct ProtocolPositionState {
    pub bump: u8,
    pub pool_id: Pubkey,
    pub tick_lower_index: i32,
    pub tick_upper_index: i32,
    pub liquidity: u128,
    pub fee_growth_inside_0_last_x64: u128,
    pub fee_growth_inside_1_last_x64: u128,
    pub token_fees_owed_0: u64,
    pub token_fees_owed_1: u64,
    pub recent_epoch: u64,
}

impl ProtocolPositionState {
    pub fn update(
        &mut self,
        tick_lower_index: i32,
        tick_upper_index: i32,
        liquidity_delta: i128,
        fee_growth_inside_0_x64: u128,
        fee_growth_inside_1_x64: u128,
    ) -> Result<()> {
        if self.liquidity == 0 && liquidity_delta == 0 {
            return Ok(());
        }

        require!(
            tick_lower_index >= MIN_TICK && tick_lower_index <= MAX_TICK,
            ErrorCode::InvalidTickRange
        );
        require!(
            tick_upper_index >= MIN_TICK && tick_upper_index <= MAX_TICK,
            ErrorCode::InvalidTickRange
        );

        // calculate accumulated Fees
        let tokens_owed_0 =
            U128::from(fee_growth_inside_0_x64.saturating_sub(self.fee_growth_inside_0_last_x64))
                .mul_div_floor(U128::from(self.liquidity), U128::from(fixed_point_64::Q64))
                .unwrap()
                .to_underflow_u64();
        let tokens_owed_1 =
            U128::from(fee_growth_inside_1_x64.saturating_sub(self.fee_growth_inside_1_last_x64))
                .mul_div_floor(U128::from(self.liquidity), U128::from(fixed_point_64::Q64))
                .unwrap()
                .to_underflow_u64();

        self.liquidity = if liquidity_delta < 0 {
            self.liquidity - u128::try_from(-liquidity_delta).unwrap()
        } else {
            self.liquidity + u128::try_from(liquidity_delta).unwrap()
        };

        self.fee_growth_inside_0_last_x64 = fee_growth_inside_0_x64;
        self.fee_growth_inside_1_last_x64 = fee_growth_inside_1_x64;
        self.tick_lower_index = tick_lower_index;
        self.tick_upper_index = tick_upper_index;

        if tokens_owed_0 > 0 || tokens_owed_1 > 0 {
            self.token_fees_owed_0 = self.token_fees_owed_0.checked_add(tokens_owed_0).unwrap();
            self.token_fees_owed_1 = self.token_fees_owed_1.checked_add(tokens_owed_1).unwrap();
        }
        self.recent_epoch = get_recent_epoch()?;

        Ok(())
    }
}

#[error_code]
pub enum ErrorCode {
    #[msg("Invalid tick range")]
    InvalidTickRange,
}
