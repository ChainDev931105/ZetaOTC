use anchor_lang::prelude::*;
use anchor_spl::token::{self, Burn, CloseAccount, Token, TokenAccount, Transfer};

declare_id!("3ruCKuy5gkAj69A4cvapM6rpeKYbvQvt6esuoC14UZNR");

// seeds
pub const STATE_SEED: &str = "state";
pub const AUCTION_SEED: &str = "auction";
pub const UNDERLYING_SEED: &str = "underlying";
pub const AUCTION_ACCOUNT_SEED: &str = "auction-account";

#[program]
pub mod zeta_auction {
    use super::*;

    pub fn initialize_state(
        ctx: Context<InitializeState>,
        args: InitializeStateArgs,
    ) -> ProgramResult {
        ctx.accounts.state.state_nonce = args.state_nonce;
        ctx.accounts.state.admin = ctx.accounts.admin.key();
        Ok(())
    }

    pub fn initialize_underlying(
        ctx: Context<InitializeUnderlying>,
        args: InitializeUnderlyingArgs,
    ) -> ProgramResult {
        if ctx.accounts.state.admin != ctx.accounts.admin.key() {
            return Err(ErrorCode::UnauthorizedAdmin.into())
        }
        ctx.accounts.underlying.underlying_nonce = args.underlying_nonce;
        Ok(())
    }

    pub fn initialize_auction(
        ctx: Context<InitializeAuction>,
        args: InitializeAuctionArgs,
    ) -> ProgramResult {
        let clock = Clock::get()?;
        if clock.unix_timestamp > args.bid_end_time as i64 {
            return Err(ErrorCode::AuctionEndTimeMustBeInTheFuture.into());
        }

        let auction_account = &mut ctx.accounts.auction_account;

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
pub struct InitializeState<'info> {
    #[account(
        init,
        seeds = [STATE_SEED.as_bytes().as_ref()],
        bump = args.state_nonce,
        payer = admin
    )]
    pub state: Box<Account<'info, State>>,
    pub system_program: Program<'info, System>,
    #[account(mut)]
    pub admin: Signer<'info>,
}

#[derive(Accounts)]
#[instruction(args: InitializeUnderlyingArgs)]
pub struct InitializeUnderlying<'info> {
    pub state: Account<'info, State>,
    #[account(
        init,
        seeds = [UNDERLYING_SEED.as_bytes().as_ref()],
        bump = args.underlying_nonce,
        payer = admin,
    )]
    pub underlying: Account<'info, Underlying>,
    #[account(mut)]
    pub admin: Signer<'info>,
    pub system_program: Program<'info, System>,
    pub token_program: Program<'info, Token>,
    pub rent: Sysvar<'info, Rent>,
}

#[derive(Accounts)]
#[instruction(args: InitializeAuctionArgs)]
pub struct InitializeAuction<'info> {
    pub state: Box<Account<'info, State>>,
    #[account(
        mut,
        seeds = [UNDERLYING_SEED.as_bytes().as_ref()],
        bump = underlying.underlying_nonce,
    )]
    pub underlying: Box<Account<'info, Underlying>>,
    #[account(
        mut,
        constraint = underlying_token_account.owner == creator.key() @ ErrorCode::InvalidTokenAccountOwner,
        constraint = underlying_token_account.amount >= args.escrow_amount @ ErrorCode::InsufficientFunds,
    )]
    pub underlying_token_account: Box<Account<'info, TokenAccount>>,
    #[account(mut)]
    pub creator: Signer<'info>,
    #[account(
        init,
        seeds = [AUCTION_ACCOUNT_SEED.as_bytes().as_ref(), underlying.key().as_ref(), &underlying.count.to_le_bytes()],
        bump = args.auction_account_nonce,
        payer = creator,
    )]
    pub auction_account: Box<Account<'info, AuctionAccount>>,
    #[account(mut)]
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
    pub state_nonce: u8,
}

#[derive(AnchorSerialize, AnchorDeserialize)]
pub struct InitializeUnderlyingArgs {
    pub underlying_nonce: u8,
}

#[derive(AnchorSerialize, AnchorDeserialize)]
pub struct InitializeAuctionArgs {
    pub escrow_amount: u64,
    pub starting_price: u64,
    pub bid_end_time: u64,
    pub cooldown_period: u64,
    pub auction_account_nonce: u8,
    pub underlying_token_nonce: u8,
    pub bid_token_nonce: u8,
}

#[account]
#[derive(Default)]
pub struct AuctionAccount {
    pub auction_account_nonce: u8,
}

#[account]
#[derive(Default)]
pub struct Underlying {
    pub underlying_nonce: u8,
    pub count: u64,
}

#[account]
#[derive(Default)]
pub struct State {
    pub state_nonce: u8,
    pub admin: Pubkey,
}

#[error]
pub enum ErrorCode {
    #[msg("Unauthorized admin")]
    UnauthorizedAdmin,
    #[msg("Invalid token account owner")]
    InvalidTokenAccountOwner,
    #[msg("Insufficient funds")]
    InsufficientFunds,
    #[msg("Auction endtime must be in the future")]
    AuctionEndTimeMustBeInTheFuture,
}
