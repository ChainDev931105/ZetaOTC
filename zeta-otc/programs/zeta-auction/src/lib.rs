use anchor_lang::prelude::*;
use anchor_spl::token::{self, Burn, CloseAccount, Mint, MintTo, Token, TokenAccount, Transfer};

declare_id!("3ruCKuy5gkAj69A4cvapM6rpeKYbvQvt6esuoC14UZNR");

// seeds
pub const ESCROW_SEED: &str = "escrow";

#[program]
pub mod zeta_auction {
    use super::*;

    pub fn initialize_state(
        ctx: Context<InitializeState>,
        args: InitializeStateArgs,
    ) -> ProgramResult {
        Ok(())
    }

    pub fn initialize_auction(
        ctx: Context<InitializeAuction>,
        args: InitializeAuctionArgs,
    ) -> ProgramResult {
        Ok(())
    }

    pub fn place_bid(
        ctx: Context<PlaceBid>
    ) -> ProgramResult {
        Ok(())
    }

    pub fn cancel_bid(
        ctx: Context<CancelBid>
    ) -> ProgramResult {
        Ok(())
    }

    pub fn withdraw_collateral(
        ctx: Context<WithdrawCollateral>
    ) -> ProgramResult {
        Ok(())
    }

    pub fn accept_bid(
        ctx: Context<AcceptBid>
    ) -> ProgramResult {
        Ok(())
    }

    pub fn terminate_auction(
        ctx: Context<TerminateAuction>
    ) -> ProgramResult {
        Ok(())
    }
}

// accounts
#[derive(Accounts)]
#[instruction(args: InitializeStateArgs)]
pub struct InitializeState {
}

#[derive(Accounts)]
#[instruction(args: InitializeAuctionArgs)]
pub struct InitializeAuction<'info> {
    pub underlying_token_account: Box<Account<'info, TokenAccount>>,
    pub bid_token_account: Box<Account<'info, TokenAccount>>,

    pub system_program: Program<'info, System>,
    pub token_program: Program<'info, Token>,
    pub rent: Sysvar<'info, Rent>,
}

#[derive(Accounts)]
pub struct PlaceBid {
}

#[derive(Accounts)]
pub struct CancelBid {
}

#[derive(Accounts)]
pub struct WithdrawCollateral {
}

#[derive(Accounts)]
pub struct AcceptBid {
}

#[derive(Accounts)]
pub struct TerminateAuction {
}

// args
#[derive(AnchorSerialize, AnchorDeserialize)]
pub struct InitializeStateArgs {
    pub amount: u64,
    pub starting_price: u64,
    pub bid_end_time: u64,
    pub cooldown_period: u64,
    pub underlying_token_nonce: u8,
    pub bid_token_nonce: u8,
}

#[derive(AnchorSerialize, AnchorDeserialize)]
pub struct InitializeAuctionArgs {
}

#[error]
pub enum ErrorCode {
    #[msg("Unauthorized admin")]
    UnauthorizedAdmin,
}
