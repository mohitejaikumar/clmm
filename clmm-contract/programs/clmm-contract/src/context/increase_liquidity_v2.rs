use anchor_lang::prelude::*;
use anchor_spl::{token::Token, token_2022::Token2022, token_interface::TokenAccount};

use crate::{
    helpers::increase_liquidity,
    state::{PersonalPositionState, PoolState, ProtocolPositionState, TickArrayState},
};

#[derive(Accounts)]
pub struct IncreaseLiquidity<'info> {
    pub nft_owner: Signer<'info>,

    #[account(
        constraint = nft_account.mint == personal_position.nft_mint,
        constraint = nft_account.amount == 1,
        token::authority = nft_owner
    )]
    pub nft_account: Box<InterfaceAccount<'info, TokenAccount>>,

    #[account(
        mut,
        constraint = personal_position.pool_id == pool_state.key()
    )]
    pub personal_position: Box<Account<'info, PersonalPositionState>>,

    #[account(mut)]
    pub pool_state: AccountLoader<'info, PoolState>,

    #[account(
        mut,
        seeds = [
            b"protocol_position",
            pool_state.key().as_ref(),
            &personal_position.tick_lower_index.to_be_bytes(),
            &personal_position.tick_upper_index.to_be_bytes()
        ],
        bump,
        constraint = protocol_position.pool_id == pool_state.key()
    )]
    pub protocol_position: Box<Account<'info, ProtocolPositionState>>,

    #[account(
        mut,
        constraint = tick_array_lower.load()?.pool_id == pool_state.key()
    )]
    pub tick_array_lower: AccountLoader<'info, TickArrayState>,

    #[account(
        mut,
        constraint = tick_array_upper.load()?.pool_id == pool_state.key()
    )]
    pub tick_array_upper: AccountLoader<'info, TickArrayState>,

    #[account(
        mut,
        token::mint = token_vault_0.mint
    )]
    pub token_account_0: Box<InterfaceAccount<'info, TokenAccount>>,

    #[account(
        mut,
        token::mint = token_vault_1.mint
    )]
    pub token_account_1: Box<InterfaceAccount<'info, TokenAccount>>,

    #[account(
        mut,
        constraint = token_vault_0.key() == pool_state.load()?.token_vault_0
    )]
    pub token_vault_0: Box<InterfaceAccount<'info, TokenAccount>>,

    #[account(
        mut,
        constraint = token_vault_1.key() == pool_state.load()?.token_vault_1
    )]
    pub token_vault_1: Box<InterfaceAccount<'info, TokenAccount>>,

    pub token_program: Program<'info, Token>,
    pub token_program_2022: Program<'info, Token2022>,

    #[account(
        address = token_vault_0.mint
    )]
    pub vault_0_mint: Box<InterfaceAccount<'info, Mint>>,

    #[account(
        address = token_vault_1.mint
    )]
    pub vault_1_mint: Box<InterfaceAccount<'info, Mint>>,
}

impl<'info> IncreaseLiquidity<'info> {
    pub fn increase_liquidity_v2<'info>(
        &mut self,
        liquidity: u128,
        amount_0_max: u64,
        amount_1_max: u64,
        base_flag: Option<bool>,
    ) -> Result<()> {
        increase_liquidity(
            &ctx.accounts.nft_owner,
            &ctx.accounts.pool_state,
            &mut ctx.accounts.protocol_position,
            &mut ctx.accounts.personal_position,
            &ctx.accounts.tick_array_lower,
            &ctx.accounts.tick_array_upper,
            &ctx.accounts.token_account_0.to_account_info(),
            &ctx.accounts.token_account_1.to_account_info(),
            &ctx.accounts.token_vault_0.to_account_info(),
            &ctx.accounts.token_vault_1.to_account_info(),
            &ctx.accounts.token_program,
            Some(&ctx.accounts.token_program_2022),
            Some(ctx.accounts.vault_0_mint.clone()),
            Some(ctx.accounts.vault_1_mint.clone()),
            &ctx.remaining_accounts,
            liquidity,
            amount_0_max,
            amount_1_max,
            base_flag,
        )
    }
}
