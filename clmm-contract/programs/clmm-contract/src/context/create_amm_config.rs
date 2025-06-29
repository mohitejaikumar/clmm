use anchor_lang::prelude::*;

use crate::state::AmmConfig;

pub const ID: Pubkey = pubkey!("GThUX1Atko4tqhN2NaiTazWSeFWMuiUvfFnyJyUghFMJ");

#[derive(Accounts)]
#[instruction(index: u16)]
pub struct CreateAmmConfig<'info> {
    #[account(
        mut,
        address = ID,
    )]
    pub owner: Signer<'info>,

    #[account(
        init,
        payer = owner,
        space = 8 + AmmConfig::INIT_SPACE,
        seeds = [b"amm_config", index.to_le_bytes().as_ref()],
        bump,
    )]
    pub amm_config: Account<'info, AmmConfig>,

    pub system_program: Program<'info, System>,
}

impl<'info> CreateAmmConfig<'info> {
    pub fn create_amm_config(
        &mut self,
        index: u16,
        tick_spacing: u16,
        protocol_fee_rate: u32,
        trade_fee_rate: u32,
        fund_fee_rate: u32,
        bumps: &CreateAmmConfigBumps,
    ) -> Result<()> {
        let amm_config = &mut self.amm_config;
        amm_config.owner = self.owner.key();
        amm_config.bump = bumps.amm_config;
        amm_config.index = index;
        amm_config.trade_fee_rate = trade_fee_rate;
        amm_config.fund_fee_rate = fund_fee_rate;
        amm_config.protocol_fee_rate = protocol_fee_rate;
        amm_config.tick_spacing = tick_spacing;
        amm_config.fund_owner = self.owner.key();

        Ok(())
    }
}
