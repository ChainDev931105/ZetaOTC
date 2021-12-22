use anchor_lang::prelude::*;

declare_id!("3ruCKuy5gkAj69A4cvapM6rpeKYbvQvt6esuoC14UZNR");

#[program]
pub mod zeta_auction {
    use super::*;
    pub fn initialize(ctx: Context<Initialize>) -> ProgramResult {
        Ok(())
    }
}

#[derive(Accounts)]
pub struct Initialize {}
