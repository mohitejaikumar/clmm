use std::ops::DerefMut;

use crate::{helpers::{add_liquidity, check_tick_array_start_index, mint_nft_and_remove_mint_authority}, state::{personal_position, PersonalPositionState, PoolState, ProtocolPositionState, TickArrayState}};
use anchor_lang::{prelude::*, system_program::{create_account, CreateAccount}};
use anchor_spl::{
    associated_token::{create, AssociatedToken, Create}, token::initialize_mint2, token_2022::{ spl_token_2022::{self, extension::{metadata_pointer, ExtensionType}, instruction::initialize_mint_close_authority}, Token2022}, token_interface::{Mint, TokenAccount}
};

#[derive(Accounts)]
#[instruction(
    tick_lower_index: i32,
    tick_upper_index: i32,
    tick_array_lower_start_index: i32,
    tick_array_upper_start_index: i32
)]
pub struct OpenPositionWithToken22Nft<'info> {
    #[account(mut)]
    pub payer: Signer<'info>,

    pub position_nft_owner: UncheckedAccount<'info>,

    #[account(mut)]
    pub position_nft_mint: Signer<'info>,

    /// ATA address where position NFT will be minted, initialize in contract
    #[account(mut)]
    pub position_nft_account: UncheckedAccount<'info>,

    #[account(mut)]
    pub pool_state: AccountLoader<'info, PoolState>,

    #[account(
        init_if_needed,
        seeds = [
            b"protocol_position",
            pool_state.key().as_ref(),
            &tick_lower_index.to_be_bytes(),
            &tick_upper_index.to_be_bytes()
        ],
        bump,
        space = 8 + ProtocolPositionState::INIT_SPACE,
        payer = payer
    )]
    pub protocol_position: Box<Account<'info, ProtocolPositionState>>,

    #[account(
        init,
        seeds = [b"personal_position", position_nft_mint.key().as_ref()],
        bump,
        payer = payer,
        space = 8 + PersonalPositionState::INIT_SPACE
    )]
    pub personal_position: Box<Account<'info, PersonalPositionState>>,

    #[account(
        mut,
        seeds = [
            b"tick_array",
            pool_state.key().as_ref(),
            &tick_array_lower_start_index.to_be_bytes()
        ],
        bump
    )]
    pub tick_array_lower: UncheckedAccount<'info>,

    #[account(
        mut,
        seeds = [
            b"tick_array",
            pool_state.key().as_ref(),
            &tick_array_upper_start_index.to_be_bytes()
        ],
        bump
    )]
    pub tick_array_upper: UncheckedAccount<'info>,

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

    pub rent: Sysvar<'info, Rent>,
    pub token_program: Program<'info, Token>,
    pub system_program: Program<'info, System>,
    pub associated_token_program: Program<'info, AssociatedToken>, // NFT ATA
    pub token_program_2022: Program<'info, Token2022>,             // for token22 mint

    #[account(
        address = token_vault_0.mint
    )]
    pub vault_0_mint: Box<InterfaceAccount<'info, Mint>>,

    #[account(
        address = token_vault_1.mint
    )]
    pub vault_1_mint: Box<InterfaceAccount<'info, Mint>>,
}

pub fn create_position_nft_mint_with_extensions<'info>(
    payer: &Signer<'info>,
    position_nft_mint: &AccountInfo<'info>,
    mint_authority: &AccountInfo<'info>,
    mint_close_authority: &AccountInfo<'info>,
    system_program: &Program<'info, System>,
    token_2022_program: &Program<'info, Token2022>,
    with_metadata: bool,
) -> Result<()> {
    let extensions = if with_metadata {
        [
            ExtensionType::MintCloseAuthority,
            ExtensionType::MetadataPointer,
        ]
        .to_vec()
    } else {
        [ExtensionType::MintCloseAuthority].to_vec()
    };
    let space = ExtensionType::try_calculate_account_len<spl_token_2022::state::Mint>(&extensions)?;
    let lamports = Rent::get()?.minimum_balance(space);

    let create_account_cpi_context = CpiContext::new(
        system_program.to_account_info(),
        CreateAccount{
            from: payer.to_account_info(),
            to: position_nft_mint.to_account_info()
        }
    );
    // create MINT ACCOUNT
    create_account(create_account_cpi_context, lamports, space.try_into().unwrap()?, token_2022_program.key )?;

    // Initialize token extensions
    for e in extensions {
        match e {
            ExtensionType::MetadataPointer => {
                let ix = metadata_pointer::instruction::initialize(
                    token_2022_program.key,
                    position_nft_mint.key,
                    None,
                    Some(position_nft_mint.key())
                )?;
                invoke(
                    &ix,
                    &[   
                        token_2022_program.to_account_info(),
                        position_nft_mint.to_account_info()
                    ]
                )?;

            }
            ExtensionType::MintCloseAuthority => {
                let ix = initialize_mint_close_authority(
                    token_2022_program.key,
                    position_nft_mint.key,
                    Some(mint_close_authority.key)
                )?;
                invoke(
                    &ix,
                    &[
                        token_2022_program.to_account_info(),
                        position_nft_mint.to_account_info()
                    ]
                )?;
            }
            _ => {
                return err!(ErrorCode::InvalidExtensionType);
            }
        }
    }

    initialize_mint2(
        CpiContext::new(
            token_2022_program.to_account_info(),
            InitializeMint2 {
                mint: position_nft_mint.to_account_info()
            }
        ),
        0,
        &mint_authority.key,
        None
    )?;


}

pub fn open_position<'b, 'info>(
   payer: &'b Signer<'info>,
   position_nft_owner: &'b UncheckedAccount<'info>,
   position_nft_mint: &'b AccountInfo<'info>,
   position_nft_account: &'b AccountInfo<'info>,
   metadata_account: Option<&'b UncheckedAccount<'info>>,
   pool_state_loader: &'b AccountLoader<'info, PoolState>,
   tick_array_lower_loader: &'b UncheckedAccount<'info>,
   tick_array_upper_loader: &'b UncheckedAccount<'info>,
   protocol_position: &'b mut Box<Account<'info, ProtocolPositionState>>,
   personal_position: &'b mut Box<Account<'info, PersonalPositionState>>,
   token_account_0: &'b AccountInfo<'info>,
   token_account_1: &'b AccountInfo<'info>,
   token_vault_0: &'b AccountInfo<'info>,
   token_vault_1: &'b AccountInfo<'info>,
   rent: &'b Sysvar<'info, Rent>,
   system_program: &'b Program<'info, System>,
   token_program: &'b Program<'info, Token>,
   _associated_token_program: &'b Program<'info, AssociatedToken>,
   metadata_program: Option<&'b Program<'info, Metadata>>,
   token_program_2022: Option<&'b Program<'info, Token2022>>,
   vault_0_mint: Option<Box<InterfaceAccount<'info, Mint>>>,
   vault_1_mint: Option<Box<InterfaceAccount<'info, Mint>>>,
   remaining_accounts: &'b [AccountInfo<'info>],
   protocol_position_bump: u8,
   personal_position_bump: u8,
   liquidity: u128,
   amount_0_max: u64,
   amount_1_max: u64,
   tick_lower_index: i32,
   tick_upper_index: i32,
   tick_array_lower_start_index: i32,
   tick_array_upper_start_index: i32,
   with_metadata: bool,
   base_flag: Option<bool>,
   use_metadata_extension: bool
) -> Result<()> {
    let mut liquidity = liquidity;

    {
        let pool_state = &mut pool_state_loader.load_mut()?;
        // check ticks order
        require!(tick_lower_index < tick_upper_index, ErrorCode::InvalidTickOrder);
        // check tick array start index
        check_tick_array_start_index(
            tick_array_lower_start_index,
            tick_lower_index,
            pool_state.tick_spacing
        )?;
        check_tick_array_start_index(
            tick_array_upper_start_index,
            tick_upper_index,
            pool_state.tick_spacing
        )?;

        let tick_array_lower_loader = TickArrayState::get_or_create_tick_array(
            payer.to_account_info(),
            tick_array_lower_loader.to_account_info(),
            tick_array_lower_start_index,
            pool_state.tick_spacing,
            &pool_state_loader,
            system_program.to_account_info(),
        )?;

        let tick_array_upper_loader = 
        if tick_array_lower_start_index == tick_array_upper_start_index {
            AccountLoader::<TickArrayState>::try_from(&tick_array_upper_loader.to_account_info())?;
        } else {
            TickArrayState::get_or_create_tick_array(
                payer.to_account_info(),
                tick_array_upper_loader.to_account_info(),
                tick_array_upper_start_index,
                pool_state.tick_spacing,
                &pool_state_loader,
                system_program.to_account_info(),
            )?;
        };

        // check if protocol position is initialized , protocol initialize also add ticks to tick array
        let protocol_position = protocol_position.deref_mut();
        if protocol_position.pool_id == Pubkey::default() {
            protocol_position.bump = protocol_position_bump;
            protocol_position.pool_id = pool_state_loader.key();
            protocol_position.tick_lower_index = tick_lower_index;
            protocol_position.tick_upper_index = tick_upper_index;
            
            tick_array_lower_loader
                .load_mut()?
                .get_tick_state_mut(tick_lower_index, pool_state.tick_spacing)?
                .tick = tick_lower_index;

            tick_array_upper_loader
                .load_mut()?
                .get_tick_state_mut(tick_upper_index, pool_state.tick_spacing)?
                .tick = tick_upper_index;
        }

        let use_tickarray_bitmap_extension = 
            pool_state.is_overflow_default_tickarray_bitmap(
                vec![tick_array_lower_start_index, tick_array_upper_start_index]
            );

        // Checkpoint: tick_array is loaded, protocol position is initialized, lets now add liquidity
        let (amount_0, amount_1, amount_0_transfer_fee, amount_1_transfer_fee) = add_liquidity(
            payer,
            token_account_0,
            token_account_1,
            token_vault_0,
            token_vault_1,
            &tick_array_lower_loader,
            &tick_array_upper_loader,
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
            tick_lower_index,
            tick_upper_index,
            base_flag,
        )?;
        
        // initialize personal position
        personal_position.bump = [personal_position_bump];
        personal_position.nft_mint = position_nft_mint.key();
        personal_position.pool_id = pool_state_loader.key();
        personal_position.tick_lower_index = tick_lower_index;
        personal_position.tick_upper_index = tick_upper_index;
        personal_position.fee_growth_inside_0_last_x64 =
            protocol_position.fee_growth_inside_0_last_x64;
        personal_position.fee_growth_inside_1_last_x64 =
            protocol_position.fee_growth_inside_1_last_x64;
 
        personal_position.liquidity = liquidity;
    }

    mint_nft_and_remove_mint_authority(
        payer,
        pool_state_loader,
        personal_position,
        position_nft_mint,
        position_nft_account,
        metadata_account,
        metadata_program,
        token_program,
        token_program_2022,
        system_program,
        rent,
        with_metadata,
        use_metadata_extension,
    )


}



impl<'info> OpenPositionWithToken22Nft<'info> {
    pub fn open_position_with_token22_nft(
        &mut self,
        liquidity: u128,
        amount_0_max: u64,
        amount_1_max: u64,
        tick_lower_index: i32,
        tick_upper_index: i32,
        tick_array_lower_start_index: i32,
        tick_array_upper_start_index: i32,
        with_metadata: bool,
        base_flag: Option<bool>,
    ) -> Result<()> {
        // create nft mint with extensions
        // create user position nft account
        // open position

        create_position_nft_mint_with_extensions(
            &self.payer,
            &self.position_nft_mint.to_account_info(),
            &self.pool_state.to_account_info(),
            &self.personal_position.to_account_info(),
            &self.system_program,
            &self.token_program_2022,
            with_metadata
        )?;

        create(CpiContext::new(
            &self.associated_token_program.to_account_info(),
            Create {
                payer: &self.payer.to_account_info(),
                associated_token: &self.position_nft_account.to_account_info(),
                authority: &self.position_nft_owner.to_account_info(),
                mint: &self.position_nft_mint.to_account_info(),
                system_program: &self.system_program.to_account_info(),
                token_program: &self.token_program_2022.to_account_info()
            }
        ))?;



        // update the pool state and personal position and protocol position
        


    }
}

#[error_code]
pub enum ErrorCode {
    #[msg("Invalid extension type")]
    InvalidExtensionType,
    #[msg("Invalid tick order")]
    InvalidTickOrder
}