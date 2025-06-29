use anchor_lang::prelude::*;
use anchor_spl::token_interface::{TokenAccount, TokenInterface};

use crate::state::{AmmConfig, PoolState};

#[derive(Accounts)]
pub struct CreatePool<'info> {
    #[account(mut)]
    pub pool_creator: Signer<'info>,

    pub amm_config: Box<Account<'info, AmmConfig>>,

    #[account(
        constraint = token_mint_0.key() < token_mint_1.key(),
        mint::token_program = token_program_0
    )]
    pub token_mint_0: Box<InterfaceAccount<'info, Mint>>,

    #[account(
        mint::token_program = token_program_1
    )]
    pub token_mint_1: Box<InterfaceAccount<'info, Mint>>,

    #[account(
        init,
        seeds = [
            b"pool",
            amm_config.key().as_ref(),
            token_mint_0.key().as_ref(),
            token_mint_1.key().as_ref(),
        ],
        bump,
        payer = pool_creator,
        space = 8 + PoolState::INIT_SPACE,
    )]
    pub pool_state: AccountLoader<'info, PoolState>,

    #[account(
        init,
        seeds = [
            b"vault",
            pool_state.key().as_ref(),
            token_mint_0.key().as_ref()
        ],
        bump,
        payer = pool_creator,
        token::mint = token_mint_0,
        token::authority = pool_state,
        token::token_program = token_program_0,
    )]
    pub token_vault_0: Box<InterfaceAccount<'info, TokenAccount>>,

    #[account(
        init,
        seeds = [
            b"vault",
            pool_state.key().as_ref(),
            token_mint_1.key().as_ref()
        ],
        bump,
        payer = pool_creator,
        token::mint = token_mint_1,
        token::authority = pool_state,
        token::token_program = token_program_1,
    )]
    pub token_vault_1: Box<InterfaceAccount<'info, TokenAccount>>,

    #[account(
        init,
        seeds = [
            b"tick_array",
            pool_state.key().as_ref(),
        ],
        bump,
        payer = pool_creator,
        space = 8 + TickArrayBitmapExtension::INIT_SPACE,
    )]
    pub tick_array_bitmap_extension: AccountLoader<'info, TickArrayBitmapExtension>,

    pub token_program_0: Interface<'info, TokenInterface>,
    pub token_program_1: Interface<'info, TokenInterface>,

    pub rent: Sysvar<'info, Rent>,

    pub system_program: Program<'info, System>,
}
