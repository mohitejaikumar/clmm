use crate::state::{PersonalPositionState, PoolState, ProtocolPositionState};
use anchor_lang::{prelude::*, system_program::CreateAccount};
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
            b"position",
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
        seeds = [b"position", position_nft_mint.key().as_ref()],
        bump,
        payer = payer,
        space = 8 + PersonalPositionState::INIT_SPACE
    )]
    pub personal_position: Box<Account<'info, PersonalPositionState>>,

    #[account(
        mut,
        seeds = [
            b"tick",
            pool_state.key().as_ref(),
            &tick_array_lower_start_index.to_be_bytes()
        ],
        bump
    )]
    pub tick_array_lower: UncheckedAccount<'info>,

    #[account(
        mut,
        seeds = [
            b"tick",
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

impl<'info> OpenPositionWithToken22Nft<'info> {
    pub fn open_position(
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
    InvalidExtensionType
}