use std::fs::Metadata;

use anchor_lang::{
    prelude::*,
    system_program::{allocate, assign, Allocate, Assign, CreateAccount, Transfer},
};
use anchor_spl::{
    metadata::{
        create_metadata_accounts_v3, mpl_token_metadata::types::DataV2, CreateMetadataAccountsV3,
    },
    token::{self, Token},
    token_2022::{
        self, get_account_data_size, initialize_account3,
        spl_token_2022::{
            self,
            extension::{
                transfer_fee::{TransferFeeConfig, MAX_FEE_BASIS_POINTS},
                BaseStateWithExtensions,
                ExtensionType::ImmutableOwner,
                StateWithExtensions,
            },
        },
        GetAccountDataSize, InitializeAccount3, Token2022,
    },
    token_interface::{spl_token_metadata_interface, Mint, TokenInterface},
};

use crate::{
    state::{PersonalPositionState, PoolState},
    util::get_recent_epoch,
};

pub fn create_or_allocate_account<'a>(
    program_id: &Pubkey,
    payer: AccountInfo<'a>,
    system_program: AccountInfo<'a>,
    target_account: AccountInfo<'a>,
    signer_seed: &[&[u8]],
    space: usize,
) -> Result<()> {
    let rent = Rent::get()?;
    let current_lamports = target_account.lamports();

    if current_lamports == 0 {
        // account is not created yet, we have to create this account
        let lamports = rent.minimum_balance(space);

        let cpi_context = CpiContext::new(
            system_program.clone(),
            CreateAccount {
                from: payer,
                to: target_account.clone(),
            },
        );
        create_account(
            cpi_context.with_signer(&[signer_seed]),
            lamports,
            u64::try_from(space).unwrap(),
            program_id,
        )?;
    } else {
        let required_lamports = rent
            .minimum_balance(space)
            .max(1)
            .saturating_sub(current_lamports);
        if required_lamports > 0 {
            let cpi_accounts = Transfer {
                from: payer.to_account_info(),
                to: target_account.clone(),
            };
            let cpi_context = CpiContext::new(system_program.clone(), cpi_accounts);
            system_program::transfer(cpi_context, required_lamports)?;
        }
        let cpi_context = CpiContext::new(
            system_program.clone(),
            Allocate {
                account_to_allocate: target_account.clone(),
            },
        );
        allocate(
            cpi_context.with_signer(&[signer_seed]),
            u64::try_from(space).unwrap(),
        )?;

        let cpi_context2 = CpiContext::new(
            system_program.clone(),
            Assign {
                account_to_assign: target_account.clone(),
            },
        );
        assign(
            cpi_context2.with_signer(&[signer_seed]),
            u64::try_from(space).unwrap(),
        )?;
    }
    Ok(())
}

pub fn create_token_vault_account<'info>(
    payer: &Signer<'info>,
    pool_state: &AccountInfo<'info>,
    token_account: &AccountInfo<'info>,
    token_mint: &InterfaceAccount<'info, Mint>,
    system_program: &Program<'info, System>,
    token_2022_program: &Interface<'info, TokenInterface>,
    signer_seeds: &[&[u8]],
) -> Result<()> {
    let immutable_owner_required = false;
    // support both spl_token_program and token_program_2022
    let cpi_context = CpiContext::new(
        token_2022_program.to_account_info(),
        GetAccountDataSize {
            mint: token_mint.to_account_info(),
        },
    );

    let extension_type = ImmutableOwner;

    let space = get_account_data_size(cpi_context, &[])?;

    create_or_allocate_account(
        token_2022_program.key,
        payer.to_account_info(),
        system_program.to_account_info(),
        token_account.to_account_info(),
        signer_seeds,
        space.try_into().unwrap(),
    )?;

    // call initializeAccount3
    initialize_account3(CpiContext::new(
        token_2022_program.to_account_info(),
        InitializeAccount3 {
            account: token_account.to_account_info(),
            mint: token_mint.to_account_info(),
            authority: pool_state.to_account_info(),
        },
    ))?;
}

// fee for input amount
pub fn get_transfer_fee(
    mint_account: Box<InterfaceAccount<Mint>>,
    pre_fee_amount: u64,
) -> Result<u64> {
    let mint_info = mint_account.to_account_info();
    if *mint_info.owner == Token::id() {
        return Ok(0);
    }

    let mint_data = mint_info.try_borrow_data()?;
    let mint = StateWithExtensions::<spl_token_2022::state::Mint>::unpack(&mint_data)?;

    let fee = if let transfer_fee_config = mint.get_extension::<TransferFeeConfig>()? {
        transfer_fee_config
            .calculate_epoch_fee(get_recent_epoch()?, pre_fee_amount)
            .unwrap()
    } else {
        0
    };

    Ok(fee)
}

pub fn get_transfer_inverse_fee(
    mint_account: Box<InterfaceAccount<Mint>>,
    post_fee_amount: u64,
) -> Result<u64> {
    let mint_info = mint_account.to_account_info();
    if *mint_info.owner == Token::id() {
        return Ok(0);
    }
    let mint_data = mint_info.try_borrow_data()?;
    let mint = StateWithExtensions::<spl_token_2022::state::Mint>::unpack(&mint_data)?;

    let fee = if let Ok(transfer_fee_config) = mint.get_extension::<TransferFeeConfig>() {
        let epoch = get_recent_epoch()?;

        let transfer_fee = transfer_fee_config.get_epoch_fee(epoch);
        if u16::from(transfer_fee.transfer_fee_basis_points) == MAX_FEE_BASIS_POINTS {
            u64::from(transfer_fee.maximum_fee)
        } else {
            let transfer_fee = transfer_fee_config
                .calculate_inverse_epoch_fee(epoch, post_fee_amount)
                .unwrap();
            let transfer_fee_for_check = transfer_fee_config
                .calculate_epoch_fee(epoch, post_fee_amount.checked_add(transfer_fee).unwrap())
                .unwrap();
            if transfer_fee != transfer_fee_for_check {
                return err!(ErrorCode::TransferFeeCalculateNotMatch);
            }
            transfer_fee
        }
    } else {
        0
    };
    Ok(fee)
}

pub fn transfer_from_user_to_pool_vault<'info>(
    signer: &Signer<'info>,
    from: &AccountInfo<'info>,
    to_vault: &AccountInfo<'info>,
    mint: Option<Box<InterfaceAccount<'info, Mint>>>,
    token_program: &AccountInfo<'info>,
    token_program_2022: Option<AccountInfo<'info>>,
    amount: u64,
) -> Result<()> {
    if amount == 0 {
        return Ok(());
    }
    let mut token_program_info = token_program.to_account_info();
    let from_token_info = from.to_account_info();
    match (mint, token_program_2022) {
        (Some(mint), Some(token_program_2022)) => {
            if from_token_info.owner == token_program_2022.key {
                token_program_info = token_program_2022.to_account_info()
            }
            token_2022::transfer_checked(
                CpiContext::new(
                    token_program_info,
                    token_2022::TransferChecked {
                        from: from_token_info,
                        to: to_vault.to_account_info(),
                        authority: signer.to_account_info(),
                        mint: mint.to_account_info(),
                    },
                ),
                amount,
                mint.decimals,
            )
        }
        _ => token::transfer(
            CpiContext::new(
                token_program_info,
                token::Transfer {
                    from: from_token_info,
                    to: to_vault.to_account_info(),
                    authority: signer.to_account_info(),
                },
            ),
            amount,
        ),
    }
}

fn get_metadata_data(personal_position_id: Pubkey) -> (String, String, String) {
    return (
        String::from("Raydium Concentrated Liquidity"),
        String::from("RCL"),
        format!(
            "https://dynamic-ipfs.raydium.io/clmm/position?id={}",
            personal_position_id.to_string()
        ),
    );
}

pub fn initialize_token_metadata_extension<'info>(
    payer: &Signer<'info>,
    position_nft_mint: &AccountInfo<'info>,
    mint_authority: &AccountInfo<'info>,
    metadata_update_authority: &AccountInfo<'info>,
    token_2022_program: &Program<'info, Token2022>,
    name: String,
    symbol: String,
    uri: String,
    signers_seeds: &[&[&[u8]]],
) -> Result<()> {
    let metadata = spl_token_metadata_interface::state::TokenMetadata {
        name,
        symbol,
        uri,
        ..Default::default()
    };

    let mint_data = position_nft_mint.try_borrow_data()?;
    let mint_state_unpacked =
        StateWithExtensions::<spl_token_2022::state::Mint>::unpack(&mint_data)?;
    let new_account_len =
        mint_state_unpacked.try_get_new_account_len_for_variable_len_extension(&metadata)?;
    let new_rent_exempt_lamports = Rent::get()?.minimum_balance(new_account_len);
    let additional_lamports = new_rent_exempt_lamports.saturating_sub(position_nft_mint.lamports());
    // CPI call will borrow the account data
    drop(mint_data);

    let cpi_context = CpiContext::new(
        token_2022_program.to_account_info(),
        Transfer {
            from: payer.to_account_info(),
            to: position_nft_mint.to_account_info(),
        },
    );
    transfer(cpi_context, additional_lamports)?;

    solana_program::program::invoke_signed(
        &spl_token_metadata_interface::instruction::initialize(
            token_2022_program.key,
            position_nft_mint.key,
            metadata_update_authority.key,
            position_nft_mint.key,
            &mint_authority.key(),
            metadata.name,
            metadata.symbol,
            metadata.uri,
        ),
        &[
            position_nft_mint.to_account_info(),
            mint_authority.to_account_info(),
            metadata_update_authority.to_account_info(),
            token_2022_program.to_account_info(),
        ],
        signers_seeds,
    )?;

    Ok(())
}

fn initialize_metadata_account<'info>(
    payer: &Signer<'info>,
    authority: &AccountInfo<'info>,
    position_nft_mint: &AccountInfo<'info>,
    metadata_account: &UncheckedAccount<'info>,
    metadata_program: &Program<'info, Metadata>,
    system_program: &Program<'info, System>,
    rent: &Sysvar<'info, Rent>,
    name: String,
    symbol: String,
    uri: String,
    signers_seeds: &[&[&[u8]]],
) -> Result<()> {
    create_metadata_accounts_v3(
        CpiContext::new_with_signer(
            metadata_program.to_account_info(),
            CreateMetadataAccountsV3 {
                metadata: metadata_account.to_account_info(),
                mint: position_nft_mint.to_account_info(),
                mint_authority: authority.to_account_info(),
                payer: payer.to_account_info(),
                update_authority: authority.to_account_info(),
                system_program: system_program.to_account_info(),
                rent: rent.to_account_info(),
            },
            signers_seeds,
        ),
        DataV2 {
            name,
            symbol,
            uri,
            seller_fee_basis_points: 0,
            creators: Some(vec![Creator {
                address: authority.key(),
                verified: true,
                share: 100,
            }]),
            collection: None,
            uses: None,
        },
        false,
        true,
        None,
    )?;
    Ok(())
}

pub fn mint_nft_and_remove_mint_authority<'info>(
    payer: &Signer<'info>,
    pool_state_loader: &AccountLoader<'info, PoolState>,
    personal_position: &Account<'info, PersonalPositionState>,
    position_nft_mint: &AccountInfo<'info>,
    position_nft_account: &AccountInfo<'info>,
    metadata_account: Option<&UncheckedAccount<'info>>,
    metadata_program: Option<&Program<'info, Metadata>>,
    token_program: &Program<'info, Token>,
    token_program_2022: Option<&Program<'info, Token2022>>,
    system_program: &Program<'info, System>,
    rent: &Sysvar<'info, Rent>,
    with_metadata: bool,
    use_metadata_extension: bool,
) -> Result<()> {
    let pool_state_info = pool_state_loader.to_account_info();
    let position_nft_mint_info = position_nft_mint.to_account_info();
    let pool_state = pool_state_loader.load()?;
    let seeds = pool_state.seeds();

    let token_program_info = if position_nft_mint_info.owner == token_program.key {
        token_program.to_account_info()
    } else {
        token_program_2022.unwrap().to_account_info()
    };

    if with_metadata {
        let (name, symbol, uri) = get_metadata_data(personal_position.key());
        if use_metadata_extension {
            initialize_token_metadata_extension(
                payer,
                &position_nft_mint_info,
                &pool_state_info,
                &personal_position.to_account_info(),
                token_program_2022.unwrap(),
                name,
                symbol,
                uri,
                &[&seeds],
            )?;
        } else {
            initialize_metadata_account(
                payer,
                &pool_state_info,
                &position_nft_mint_info,
                metadata_account.unwrap(),
                metadata_program.unwrap(),
                system_program,
                rent,
                name,
                symbol,
                uri,
                &[&seeds],
            )?;
        }
    }
    // Mint the NFT
    token_2022::mint_to(
        CpiContext::new_with_signer(
            token_program_info.to_account_info(),
            token_2022::MintTo {
                mint: position_nft_mint_info.clone(),
                to: position_nft_account.to_account_info(),
                authority: pool_state_info.clone(),
            },
            &[&seeds],
        ),
        1,
    )?;

    // Disable minting
    token_2022::set_authority(
        CpiContext::new_with_signer(
            token_program_info.to_account_info(),
            token_2022::SetAuthority {
                current_authority: pool_state_loader.to_account_info(),
                account_or_mint: position_nft_mint_info,
            },
            &[&seeds],
        ),
        AuthorityType::MintTokens,
        None,
    )
}

#[error_code]
pub enum ErrorCode {
    #[msg("Transfer fee calculate not match")]
    TransferFeeCalculateNotMatch,
}
