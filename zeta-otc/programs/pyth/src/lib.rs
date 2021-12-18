// https://github.com/project-serum/anchor/tree/master/examples/pyth
use anchor_lang::prelude::*;
pub mod pc;

declare_id!("3BkgqQAfo73bxTUDiPCD8KSGdpvsoTriR6pKrcguQCBs");

#[program]
pub mod pyth {
    use super::*;

    pub fn initialize(ctx: Context<Initialize>, price: i64, expo: i32, conf: u64) -> ProgramResult {
        let oracle = &ctx.accounts.price;

        let mut price_oracle = pc::Price::load(&oracle).unwrap();

        price_oracle.agg.price = price;
        price_oracle.agg.conf = conf;
        price_oracle.expo = expo;
        price_oracle.ptype = pc::PriceType::Price;
        Ok(())
    }

    pub fn set_price(ctx: Context<SetPrice>, price: i64) -> ProgramResult {
        let oracle = &ctx.accounts.price;
        let mut price_oracle = pc::Price::load(&oracle).unwrap();
        price_oracle.agg.price = price as i64;
        Ok(())
    }
}

#[derive(Accounts)]
pub struct SetPrice<'info> {
    #[account(mut)]
    pub price: AccountInfo<'info>,
}

#[derive(Accounts)]
pub struct Initialize<'info> {
    #[account(mut)]
    pub price: AccountInfo<'info>,
}

#[error]
pub enum ErrorCode {
    #[msg("This is an error message clients will automatically display")]
    Hello,
}
