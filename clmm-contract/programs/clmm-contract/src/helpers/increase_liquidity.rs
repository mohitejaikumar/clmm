use anchor_lang::prelude::*;
use anchor_spl::{token::Token, token_2022::Token2022, token_interface::Mint};

use crate::{
    helpers::{add_liquidity, fixed_point_64, U128},
    state::{
        personal_position, protocol_position, PersonalPositionState, PoolState,
        ProtocolPositionState, TickArrayBitmapExtension, TickArrayState,
    },
};

pub fn increase_liquidity<'b, 'info>(
    nft_owner: &'b Signer<'info>,
    pool_state_loader: &'b AccountLoader<'info, PoolState>,
    protocol_position: &'b mut Box<Account<'info, ProtocolPositionState>>,
    personal_position: &'b mut Box<Account<'info, PersonalPositionState>>,
    tick_array_lower_loader: &'b AccountLoader<'info, TickArrayState>,
    tick_array_upper_loader: &'b AccountLoader<'info, TickArrayState>,
    token_account_0: &'b AccountInfo<'info>,
    token_account_1: &'b AccountInfo<'info>,
    token_vault_0: &'b AccountInfo<'info>,
    token_vault_1: &'b AccountInfo<'info>,
    token_program: &'b Program<'info, Token>,
    token_program_2022: &'b Program<'info, Token2022>,
    vault_0_mint: Option<Box<InterfaceAccount<'info, Mint>>>,
    vault_1_mint: Option<Box<InterfaceAccount<'info, Mint>>>,
    remaining_account: &'c [AccountInfo<'info>],
    liquidity: u128,
    amount_0_max: u64,
    amount_1_max: u64,
    base_flag: Option<bool>,
) -> Result<()> {
    let mut liquidity = liquidity;
    let pool_state = &mut pool_state_loader.load_mut()?;

    let tick_lower = personal_position.tick_lower_index;
    let tick_upper = personal_position.tick_upper_index;

    let use_tickarray_bitmap_extension =
        pool_state.is_overflow_default_tickarray_bitmap(vec![tick_lower, tick_upper]);

    let (amount_0, amount_1, amount_0_transfer_fee, amount_1_transfer_fee) = add_liquidity(
        &nft_owner,
        token_account_0,
        token_account_1,
        token_vault_0,
        token_vault_1,
        &AccountLoad::<TickArrayState>::try_from(&tick_array_lower_loader.to_account_info())?,
        &AccountLoad::<TickArrayState>::try_from(&tick_array_upper_loader.to_account_info())?,
        protocol_position,
        token_program_2022,
        token_program,
        vault_0_mint,
        vault_1_mint,
        if use_tickarray_bitmap_extension {
            require_keys_eq!(
                remaining_accounts[0].key(),
                TickArrayBitmapExtension::key(pool_state_loader.key())
            );
            Some(&remaining_accounts[0])
        } else {
            None
        },
        pool_state,
        &mut liquidity,
        amount_0_max,
        amount_1_max,
        tick_lower,
        tick_upper,
        base_flag,
    )?;

    personal_position.token_fees_owed_0 = calculate_latest_token_fees(
        personal_position.token_fees_owed_0,
        personal_position.fee_growth_inside_0_last_x64,
        protocol_position.fee_growth_inside_0_last_x64,
        personal_position.liquidity,
    );
    personal_position.token_fees_owed_1 = calculate_latest_token_fees(
        personal_position.token_fees_owed_1,
        personal_position.fee_growth_inside_1_last_x64,
        protocol_position.fee_growth_inside_1_last_x64,
        personal_position.liquidity,
    );

    personal_position.fee_growth_inside_0_last_x64 = protocol_position.fee_growth_inside_0_last_x64;
    personal_position.fee_growth_inside_1_last_x64 = protocol_position.fee_growth_inside_1_last_x64;

    personal_position.liquidity = personal_position.liquidity.checked_add(liquidity).unwrap();

    Ok(())
}

pub fn calculate_latest_token_fees(
    last_total_fees: u64,
    fee_growth_inside_last_x64: u128,
    fee_growth_inside_latest_x64: u128,
    liquidity: u128,
) -> u64 {
    let fee_growth_delta =
        U128::from(fee_growth_inside_latest_x64.wrapping_sub(fee_growth_inside_last_x64))
            .mul_div_floor(U128::from(liquidity), U128::from(fixed_point_64::Q64))
            .unwrap()
            .to_underflow_u64();

    last_total_fees.checked_add(fee_growth_delta).unwrap()
}
