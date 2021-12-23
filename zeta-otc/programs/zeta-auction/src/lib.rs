use anchor_lang::prelude::*;
use anchor_spl::token::{self, Burn, CloseAccount, Mint, MintTo, Token, TokenAccount, Transfer};

declare_id!("3ruCKuy5gkAj69A4cvapM6rpeKYbvQvt6esuoC14UZNR");

// seeds
pub const STATE_SEED: &str = "state";
pub const AUCTION_SEED: &str = "auction";
pub const MINT_AUTH_SEED: &str = "mint-auth";
pub const UNDERLYING_SEED: &str = "underlying";

#[program]
pub mod zeta_auction {
    use super::*;

    pub fn initialize_state(
        ctx: Context<InitializeState>,
        args: InitializeStateArgs,
    ) -> ProgramResult {
        ctx.accounts.state.state_nonce = args.state_nonce;
        ctx.accounts.state.mint_auth_nonce = args.mint_auth_nonce;
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
    #[account(
        seeds = [MINT_AUTH_SEED.as_bytes().as_ref()],
        bump = args.mint_auth_nonce,
    )]
    pub mint_authority: AccountInfo<'info>,
}

#[derive(Accounts)]
#[instruction(args: InitializeUnderlyingArgs)]
pub struct InitializeUnderlying<'info> {
    pub state: Account<'info, State>,
    #[account(
        init,
        seeds = [UNDERLYING_SEED.as_bytes().as_ref(), mint.key().as_ref()],
        bump = args.underlying_nonce,
        payer = admin,
    )]
    pub underlying: Account<'info, Underlying>,
    pub mint: Account<'info, Mint>,
    #[account(mut)]
    pub admin: Signer<'info>,
    pub system_program: Program<'info, System>,
    pub token_program: Program<'info, Token>,
    pub rent: Sysvar<'info, Rent>,
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
    pub state_nonce: u8,
    pub mint_auth_nonce: u8,
}

#[derive(AnchorSerialize, AnchorDeserialize)]
pub struct InitializeUnderlyingArgs {
    pub underlying_nonce: u8,
}

#[derive(AnchorSerialize, AnchorDeserialize)]
pub struct InitializeAuctionArgs {
    pub amount: u64,
    pub starting_price: u64,
    pub bid_end_time: u64,
    pub cooldown_period: u64,
    pub underlying_token_nonce: u8,
    pub bid_token_nonce: u8,
}

#[account]
#[derive(Default)]
pub struct Underlying {
    pub underlying_nonce: u8,
    pub mint: Pubkey,
}

#[account]
#[derive(Default)]
pub struct State {
    pub state_nonce: u8,
    pub mint_auth_nonce: u8,
    pub admin: Pubkey,
}

#[error]
pub enum ErrorCode {
    #[msg("Unauthorized admin")]
    UnauthorizedAdmin,
}
