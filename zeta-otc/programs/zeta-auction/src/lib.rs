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
        ctx: Context<PlaceBid>,
        args: PlaceBidArgs,
    ) -> ProgramResult {
        Ok(())
    }

    pub fn cancel_bid(
        ctx: Context<CancelBid>,
        args: CancelBidArgs,
    ) -> ProgramResult {
        Ok(())
    }

    pub fn withdraw_collateral(
        ctx: Context<WithdrawCollateral>,
        args: WithdrawCollateralArgs,
    ) -> ProgramResult {
        Ok(())
    }

    pub fn accept_bid(
        ctx: Context<AcceptBid>,
        args: AcceptBidArgs,
    ) -> ProgramResult {
        Ok(())
    }

    pub fn terminate_auction(
        ctx: Context<TerminateAuction>,
        args: TerminateAuctionArgs,
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
#[instruction(args: PlaceBidArgs)]
pub struct PlaceBid {
}

#[derive(Accounts)]
#[instruction(args: CancelBidArgs)]
pub struct CancelBid {
}

#[derive(Accounts)]
#[instruction(args: WithdrawCollateralArgs)]
pub struct WithdrawCollateral {
}

#[derive(Accounts)]
#[instruction(args: AcceptBidArgs)]
pub struct AcceptBid {
}

#[derive(Accounts)]
#[instruction(args: TerminateAuctionArgs)]
pub struct TerminateAuction {
}

// args
#[derive(AnchorSerialize, AnchorDeserialize)]
pub struct InitializeStateArgs {
    pub starting_price: u64,
    pub bid_end_time: u64,
    pub cooldown_period: u64,
}

#[derive(AnchorSerialize, AnchorDeserialize)]
pub struct InitializeAuctionArgs {
}

#[derive(AnchorSerialize, AnchorDeserialize)]
pub struct PlaceBidArgs {
}

#[derive(AnchorSerialize, AnchorDeserialize)]
pub struct CancelBidArgs {
}

#[derive(AnchorSerialize, AnchorDeserialize)]
pub struct WithdrawCollateralArgs {
}

#[derive(AnchorSerialize, AnchorDeserialize)]
pub struct AcceptBidArgs {
}

#[derive(AnchorSerialize, AnchorDeserialize)]
pub struct TerminateAuctionArgs {
}
