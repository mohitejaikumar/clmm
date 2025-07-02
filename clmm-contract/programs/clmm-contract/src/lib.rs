use anchor_lang::prelude::*;
pub mod context;
pub use context::*;
pub mod helpers;
pub mod state;

declare_id!("B6QRukodumWtx6KxBRnz13D3dFgwuXe1JLyVgfyCedV6");

#[program]
pub mod clmm_contract {
    use super::*;

    pub fn create_amm_config(
        ctx: Context<CreateAmmConfig>,
        index: u16,
        tick_spacing: u16,
        protocol_fee_rate: u32,
        trade_fee_rate: u32,
        fund_fee_rate: u32,
    ) -> Result<()> {
        msg!("creating amm config: {:?}", ctx.program_id);
        ctx.accounts.create_amm_config(
            index,
            tick_spacing,
            protocol_fee_rate,
            trade_fee_rate,
            fund_fee_rate,
            &ctx.bumps,
        )?;
        Ok(())
    }
}
