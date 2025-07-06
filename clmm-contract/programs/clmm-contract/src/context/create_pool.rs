use anchor_lang::prelude::*;
use anchor_spl::{
    token::Token,
    token_2022::spl_token_2022::extension::{
        BaseStateWithExtensions, ExtensionType, StateWithExtensions,
    },
    token_interface::{Mint, TokenAccount, TokenInterface},
};

use crate::{
    helpers::{create_token_vault_account, tick::get_tick_at_sqrt_price, token::get_token_vault},
    state::{AmmConfig, PoolState, SupportMint, TickArrayBitmapExtension},
};

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
            b"tick_array_bitmap",
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

pub fn support_mint_associated_is_initialized(
    remaining_accounts: &[AccountInfo], // it may include SupportMint account
    token_mint: &InterfaceAccount<Mint>,
) -> Result<bool> {
    if remaining_accounts.len() == 0 {
        return Ok(false);
    }
    let seeds = &[b"support_mint", token_mint.key().as_ref()];
    let (if_initialized_mint_account, _bump) = Pubkey::find_program_address(seeds, &crate::id());
    let mut is_mint_initialized = false;

    for mint_account_info in remaining_accounts.iter() {
        if *mint_account_info.owner != crate::id()
            || mint_account_info.key() != if_initialized_mint_account
        {
            continue;
        }
        let mint_associated =
            SupportMint::try_deserialize(&mut mint_account_info.data.borrow().as_ref())?;

        if mint_associated.mint == token_mint.key() {
            is_mint_initialized = true;
            break;
        }
    }
    return Ok(is_mint_initialized);
}

pub fn is_mint_supported(
    mint_account: &InterfaceAccount<Mint>,
    is_mint_initialized: bool,
) -> Result<bool> {
    let mint_info = mint_account.to_account_info();
    // legacy token account is supported at first place, should check for token2022
    if mint_info.owner == Token::id() {
        return Ok(true);
    }

    if is_mint_initialized {
        return Ok(true);
    }

    // check the supported extension
    let mint_data = mint_info.try_borrow_data()?;
    let mint = StateWithExtensions::<spl_token_2022::state::Mint>::unpack(&mint_data)?;
    let extensions = mint.get_extension_types()?;
    for e in extensions {
        if e != ExtensionType::TransferFeeConfig
            && e != ExtensionType::MetadataPointer
            && e != ExtensionType::TokenMetadata
            && e != ExtensionType::InterestBearingConfig
        {
            return Ok(false);
        }
    }

    Ok(true)
}

impl<'info> CreatePool<'info> {
    pub fn create_pool(
        &mut self,
        sqrt_price_x64: u128,
        open_time: u64,
        bumps: CreatePoolBumps,
    ) -> Result<()> {
        // check if mints are initialized
        // if its not initialized, check if it is supported
        let mint0_is_initialized =
            support_mint_associated_is_initialized(&self.remaining_accounts, &self.token_mint_0)?;

        let mint1_is_initialized =
            support_mint_associated_is_initialized(&self.remaining_accounts, &self.token_mint_1)?;

        if (is_mint_supported(&self.token_mint_0, mint0_is_initialized)?
            && is_mint_supported(&self.token_mint_1, mint1_is_initialized)?)
        {
            return Err(ErrorCode::MintNotSupported);
        }

        let block_timestamp = Clock::get()?.unix_timestamp as u64;
        require_gt!(block_timestamp, open_time);

        let pool_id = self.pool_state.key();
        // load_int if first time initilized and mut ref
        // load_mut if already initialized
        let mut pool_state = self.pool_state.load_init()?;
        let tick = get_tick_at_sqrt_price(sqrt_price_x64)?;

        msg!("tick: {} price: {}", tick, sqrt_price_x64);

        // init token vault accounts
        create_token_vault_account(
            &self.pool_creator,
            &self.pool_state.to_account_info(),
            &self.token_vault_0.to_account_info(),
            &self.token_mint_0,
            &self.system_program,
            &self.token_program_0,
            &[
                b"vault",
                self.pool_state.key().as_ref(),
                self.token_mint_0.key().as_ref(),
                &[bumps.token_vault_0][..],
            ],
        )?;
        create_token_vault_account(
            &self.pool_creator,
            &self.pool_state.to_account_info(),
            &self.token_vault_1.to_account_info(),
            &self.token_mint_1,
            &self.system_program,
            &self.token_program_1,
            &[
                b"vault",
                self.pool_state.key().as_ref(),
                self.token_mint_1.key().as_ref(),
                &[bumps.token_vault_1][..],
            ],
        )?;
        pool_state.amm_config = self.amm_config.key();
        pool_state.token_mint_0 = self.token_mint_0.key();
        pool_state.token_mint_1 = self.token_mint_1.key();
        pool_state.token_vault_0 = self.token_vault_0.key();
        pool_state.token_vault_1 = self.token_vault_1.key();
        pool_state.sqrt_price_x64 = sqrt_price_x64;
        pool_state.tick_current = tick;
        pool_state.mint_decimals_0 = self.token_mint_0.decimals;
        pool_state.mint_decimals_1 = self.token_mint_1.decimals;
        pool_state.tick_spacing = self.amm_config.tick_spacing;
        pool_state.liquidity = 0;
        pool_state.fee_growth_global_0_x64 = 0;
        pool_state.fee_growth_global_1_x64 = 0;
        pool_state.protocol_fees_token_0 = 0;
        pool_state.protocol_fees_token_1 = 0;
        pool_state.swap_in_amount_token_0 = 0;
        pool_state.swap_in_amount_token_1 = 0;
        pool_state.swap_out_amount_token_0 = 0;
        pool_state.swap_out_amount_token_1 = 0;
        pool_state.owner = self.pool_creator.key();
        pool_state.bump = [bumps.pool_state];
        pool_state.open_time = open_time;
        pool_state.recent_epoch = Clock::get()?.epoch;
        pool_state.tick_array_bitmap = [0; 16];
        pool_state.total_fees_token_0 = 0;
        pool_state.total_fees_claimed_token_0 = 0;
        pool_state.total_fees_token_1 = 0;
        pool_state.total_fees_claimed_token_1 = 0;
        pool_state.fund_fees_token_0 = 0;
        pool_state.fund_fees_token_1 = 0;

        &self
            .tick_array_bitmap_extension
            .load_init()?
            .initialize(pool_id);

        msg!("pool_state: {:?}", pool_id);
        msg!(
            "tick_array_bitmap_extension: {:?}",
            &self.tick_array_bitmap_extension.key()
        );

        Ok(())
    }
}

#[error_code]
pub enum ErrorCode {
    #[msg("Mint is not supported")]
    MintNotSupported,
}
