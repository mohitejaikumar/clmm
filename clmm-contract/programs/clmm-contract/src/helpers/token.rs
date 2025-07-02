use anchor_lang::{
    prelude::*,
    system_program::{allocate, assign, Allocate, Assign, CreateAccount, Transfer},
};
use anchor_spl::{
    token_2022::{
        get_account_data_size, initialize_account3,
        spl_token_2022::extension::ExtensionType::ImmutableOwner, GetAccountDataSize,
        InitializeAccount3,
    },
    token_interface::{Mint, TokenInterface},
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
