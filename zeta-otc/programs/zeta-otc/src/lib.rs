use anchor_lang::prelude::*;
use anchor_spl::token::{self, Burn, CloseAccount, Mint, MintTo, Token, TokenAccount, Transfer};
use pyth::pc;

declare_id!("Fg6PaFpoGXkYsidMpWTK6W2BeZ7FEfcYkg476zPFsLnS");

pub const UNDERLYING_SEED: &str = "underlying";
pub const STATE_SEED: &str = "state";
pub const MINT_AUTH_SEED: &str = "mint-auth";
pub const VAULT_AUTH_SEED: &str = "vault-auth";
pub const VAULT_SEED: &str = "vault";
pub const OPTION_ACCOUNT_SEED: &str = "option-account";
pub const OPTION_MINT_SEED: &str = "option-mint";
pub const OPTION_MINT_DECIMALS: u8 = 4;
pub const USDC_DECIMALS: u32 = 32;

#[program]
pub mod zeta_otc {
    use super::*;

    pub fn initialize_state(
        ctx: Context<InitializeState>,
        args: InitializeStateArgs,
    ) -> ProgramResult {
        ctx.accounts.state.state_nonce = args.state_nonce;
        ctx.accounts.state.mint_auth_nonce = args.mint_auth_nonce;
        ctx.accounts.state.vault_auth_nonce = args.vault_auth_nonce;
        ctx.accounts.state.admin = ctx.accounts.admin.key();
        Ok(())
    }

    pub fn initialize_underlying(
        ctx: Context<InitializeUnderlying>,
        args: InitializeUnderlyingArgs,
    ) -> ProgramResult {
        if ctx.accounts.state.admin != ctx.accounts.admin.key() {
            return Err(ErrorCode::UnauthorizedAdmin.into());
        }
        ctx.accounts.underlying.underlying_nonce = args.underlying_nonce;
        ctx.accounts.underlying.mint = ctx.accounts.mint.key();
        ctx.accounts.underlying.oracle = ctx.accounts.oracle.key();
        Ok(())
    }

    // TODO sense check the strike.
    pub fn initialize_option(
        ctx: Context<InitializeOption>,
        args: InitializeOptionArgs,
    ) -> ProgramResult {
        let clock = Clock::get()?;
        if clock.unix_timestamp > args.expiry as i64 {
            return Err(ErrorCode::OptionExpirationMustBeInTheFuture.into());
        }

        let option_account = &mut ctx.accounts.option_account;
        option_account.option_account_nonce = args.option_account_nonce;
        option_account.option_mint_nonce = args.option_mint_nonce;
        option_account.creator_option_token_account_nonce = args.token_account_nonce;
        option_account.vault_nonce = args.vault_nonce;

        option_account.option_mint = ctx.accounts.option_mint.key();
        option_account.underlying_mint = ctx.accounts.underlying_mint.key();
        option_account.creator = ctx.accounts.creator.key();
        option_account.strike = args.strike;
        option_account.expiry = args.expiry;

        option_account.underlying_count = ctx.accounts.underlying.count;
        ctx.accounts.underlying.count = ctx.accounts.underlying.count.checked_add(1).unwrap();

        let mint_seeds = mint_authority! {
            bump = ctx.accounts.state.mint_auth_nonce
        };

        let collateral_min_lot_size: u64 =
            get_token_amount_per_option(&ctx.accounts.underlying_mint);
        assert!(args.collateral_amount % collateral_min_lot_size == 0);

        let mint_amount = args
            .collateral_amount
            .checked_div(collateral_min_lot_size)
            .unwrap();

        token::mint_to(
            ctx.accounts
                .into_mint_to_context()
                .with_signer(&[&mint_seeds[..]]),
            mint_amount,
        )?;

        token::transfer(ctx.accounts.into_transfer_context(), args.collateral_amount)?;

        Ok(())
    }

    pub fn burn_option(ctx: Context<BurnOption>, amount: u64) -> ProgramResult {
        let clock = Clock::get()?;

        if clock.unix_timestamp > ctx.accounts.option_account.expiry as i64 {
            return Err(ErrorCode::CannotBurnOptionsAfterExpiry.into());
        }

        if ctx.accounts.option_account.settlement_price != 0 {
            return Err(ErrorCode::CannotBurnOptionsAfterSettlementPriceIsSet.into());
        }

        let mint_seeds = mint_authority! {
            bump = ctx.accounts.state.mint_auth_nonce
        };

        let vault_seeds = vault_authority! {
            bump = ctx.accounts.state.vault_auth_nonce
        };

        let underlying_min_lot_size: u64 = 10u64
            .pow(ctx.accounts.underlying_mint.decimals.into())
            .checked_div(10u64.pow(OPTION_MINT_DECIMALS.into()))
            .unwrap();

        let underlying_amount = amount.checked_mul(underlying_min_lot_size).unwrap();

        token::burn(
            ctx.accounts
                .into_burn_context()
                .with_signer(&[&mint_seeds[..]]),
            amount,
        )?;

        token::transfer(
            ctx.accounts
                .into_transfer_context()
                .with_signer(&[&vault_seeds[..]]),
            underlying_amount,
        )?;

        Ok(())
    }

    pub fn set_settlement_price(ctx: Context<SetSettlementPrice>) -> ProgramResult {
        let clock = Clock::get()?;
        let option_account = &ctx.accounts.option_account;

        let start = option_account
            .expiry
            .checked_sub(ctx.accounts.state.settlement_price_threshold_seconds)
            .unwrap();

        let end = option_account
            .expiry
            .checked_add(ctx.accounts.state.settlement_price_threshold_seconds)
            .unwrap();

        if clock.unix_timestamp < start as i64 {
            msg!(
                "Current time {} < Settlement start {}",
                clock.unix_timestamp,
                start
            );
            return Err(ErrorCode::BeforeSetSettlementPriceTime.into());
        }

        if clock.unix_timestamp > end as i64 {
            msg!(
                "Current time {} > Settlement end {}",
                clock.unix_timestamp,
                end
            );
            return Err(ErrorCode::AfterSetSettlementPriceTime.into());
        }

        if ctx.accounts.option_account.settlement_price != 0 {
            msg!(
                "Settlement price set at {}",
                ctx.accounts.option_account.settlement_price
            );
            return Err(ErrorCode::SettlementPriceAlreadySet.into());
        }

        let oracle_price = get_oracle_price(&ctx.accounts.oracle) as u64;

        ctx.accounts.option_account.settlement_price = oracle_price;
        // ctx.accounts.option_account.mint_supply_at_settlement = ctx.accounts.option_mint.supply;

        // If the option has expired worthless
        if oracle_price <= ctx.accounts.option_account.strike {
        } else {
            let itm_amount = oracle_price
                .checked_sub(ctx.accounts.option_account.strike)
                .unwrap();

            // 100_000
            let token_amount_per_option =
                get_token_amount_per_option(&ctx.accounts.underlying_mint);

            // (Oracle spot - strike) / oracle_spot) * units of underlying per option
            let profit_per_option = token_amount_per_option
                .checked_mul(itm_amount)
                .unwrap()
                .checked_div(oracle_price)
                .unwrap();

            let total_profit = profit_per_option
                .checked_mul(ctx.accounts.option_mint.supply)
                .unwrap();

            ctx.accounts.option_account.remaining_collateral =
                ctx.accounts.vault.amount.checked_sub(total_profit).unwrap();
        }

        Ok(())
    }

    pub fn close_option_account(ctx: Context<CloseOptionAccount>) -> ProgramResult {
        let option_account = &ctx.accounts.option_account;
        let clock = Clock::get()?;
        let close_timestamp = option_account.expiry;

        if clock.unix_timestamp < close_timestamp as i64 {
            msg!(
                "Clock timestamp: {}, Close timestamp: {}, Diff = {}",
                clock.unix_timestamp,
                close_timestamp,
                close_timestamp - (clock.unix_timestamp as u64)
            );

            return Err(ErrorCode::NotPastOptionCloseTime.into());
        }

        let vault_seeds = vault_authority! {
            bump = ctx.accounts.state.vault_auth_nonce
        };

        // Transfer what remains of the vault account into the creator's token account.
        token::transfer(
            ctx.accounts
                .into_transfer_context()
                .with_signer(&[&vault_seeds[..]]),
            ctx.accounts.vault.amount,
        )?;

        // Close vault account.
        token::close_account(
            ctx.accounts
                .into_close_vault_context()
                .with_signer(&[&vault_seeds[..]]),
        )?;

        token::burn(
            ctx.accounts.into_burn_context(),
            ctx.accounts.user_option_token_account.amount,
        )?;

        token::close_account(ctx.accounts.into_close_user_token_account())?;

        Ok(())
    }
}

#[derive(Accounts)]
#[instruction(args: InitializeStateArgs)]
pub struct InitializeState<'info> {
    #[account(
        init,
        seeds = [STATE_SEED.as_bytes().as_ref()],
        bump = args.state_nonce,
        payer = admin,
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
    #[account(
        seeds = [VAULT_AUTH_SEED.as_bytes().as_ref()],
        bump = args.vault_auth_nonce,
    )]
    pub vault_authority: AccountInfo<'info>,
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
    pub oracle: UncheckedAccount<'info>,
    #[account(mut)]
    pub admin: Signer<'info>,
    pub system_program: Program<'info, System>,
    pub token_program: Program<'info, Token>,
    pub rent: Sysvar<'info, Rent>,
}

#[derive(Accounts)]
#[instruction(args: InitializeOptionArgs)]
pub struct InitializeOption<'info> {
    pub state: Box<Account<'info, State>>,
    #[account(
        mut,
        seeds = [UNDERLYING_SEED.as_bytes().as_ref(), underlying_mint.key().as_ref()],
        bump = underlying.underlying_nonce,
    )]
    pub underlying: Box<Account<'info, Underlying>>,
    #[account(
        init,
        token::mint = underlying_mint,
        token::authority = vault_authority,
        seeds = [VAULT_SEED.as_bytes().as_ref(), option_account.key().as_ref()],
        bump = args.vault_nonce,
        payer = creator,
    )]
    pub vault: Account<'info, TokenAccount>,
    #[account(
        seeds = [VAULT_AUTH_SEED.as_bytes().as_ref()],
        bump = state.vault_auth_nonce,
    )]
    pub vault_authority: AccountInfo<'info>,
    pub underlying_mint: Box<Account<'info, Mint>>,
    #[account(
        mut,
        constraint = underlying_token_account.mint == underlying_mint.key() @ ErrorCode::TokenAccountMintMismatch,
        constraint = underlying_token_account.owner == creator.key() @ ErrorCode::InvalidTokenAccountOwner,
        constraint = underlying_token_account.amount >= args.collateral_amount @ ErrorCode::InsufficientFunds,
    )]
    pub underlying_token_account: Box<Account<'info, TokenAccount>>,
    #[account(mut)]
    pub creator: Signer<'info>,
    #[account(
        init,
        seeds = [OPTION_ACCOUNT_SEED.as_bytes().as_ref(), underlying.key().as_ref(), &underlying.count.to_le_bytes()],
        bump = args.option_account_nonce,
        payer = creator,
    )]
    pub option_account: Box<Account<'info, OptionAccount>>,
    #[account(
        seeds = [MINT_AUTH_SEED.as_bytes().as_ref()],
        bump = state.mint_auth_nonce,
    )]
    pub mint_authority: AccountInfo<'info>,
    #[account(
        init,
        mint::decimals = OPTION_MINT_DECIMALS,
        mint::authority = mint_authority,
        seeds = [OPTION_MINT_SEED.as_bytes().as_ref(), option_account.key().as_ref()],
        bump = args.option_mint_nonce,
        payer = creator,
    )]
    pub option_mint: Box<Account<'info, Mint>>,
    #[account(
        init,
        token::mint = option_mint,
        token::authority = creator,
        seeds = [option_mint.key().as_ref(), creator.key().as_ref()],
        bump = args.token_account_nonce,
        payer = creator,
    )]
    pub user_option_token_account: Box<Account<'info, TokenAccount>>,
    pub system_program: Program<'info, System>,
    pub token_program: Program<'info, Token>,
    pub rent: Sysvar<'info, Rent>,
}

#[derive(Accounts)]
pub struct SetSettlementPrice<'info> {
    pub state: Box<Account<'info, State>>,
    #[account(
        mut,
        seeds = [UNDERLYING_SEED.as_bytes().as_ref(), underlying_mint.key().as_ref()],
        bump = underlying.underlying_nonce,
    )]
    pub underlying: Box<Account<'info, Underlying>>,
    pub underlying_mint: Box<Account<'info, Mint>>,
    #[account(
        mut,
        seeds = [OPTION_ACCOUNT_SEED.as_bytes().as_ref(), underlying.key().as_ref(), &option_account.underlying_count.to_le_bytes()],
        bump = option_account.option_account_nonce,
    )]
    pub option_account: Box<Account<'info, OptionAccount>>,
    #[account(
        constraint = oracle.key() == underlying.oracle @ ErrorCode::InvalidOracle
    )]
    pub oracle: AccountInfo<'info>,
    #[account(
        constraint = option_mint.key() == option_account.option_mint @ ErrorCode::InvalidOracle
    )]
    pub option_mint: Account<'info, Mint>,
    #[account(
        seeds = [VAULT_SEED.as_bytes().as_ref(), option_account.key().as_ref()],
        bump = option_account.vault_nonce,
    )]
    pub vault: Account<'info, TokenAccount>,
}

#[derive(Accounts)]
pub struct SetSettlementPriceOverride<'info> {
    pub state: Box<Account<'info, State>>,
    #[account(mut)]
    pub option_account: Box<Account<'info, OptionAccount>>,
    #[account(
        constraint = admin.key() == state.admin @ ErrorCode::OnlyAdminCanOverrideSettlementPrice
    )]
    pub admin: Signer<'info>,
}

#[derive(Accounts)]
#[instruction(amount: u64)]
pub struct BurnOption<'info> {
    pub state: Box<Account<'info, State>>,
    #[account(
        seeds = [UNDERLYING_SEED.as_bytes().as_ref(), underlying_mint.key().as_ref()],
        bump = underlying.underlying_nonce,
    )]
    pub underlying: Box<Account<'info, Underlying>>,
    #[account(
        mut,
        seeds = [VAULT_SEED.as_bytes().as_ref(), option_account.key().as_ref()],
        bump = option_account.vault_nonce,
    )]
    pub vault: Box<Account<'info, TokenAccount>>,
    pub underlying_mint: Box<Account<'info, Mint>>,
    #[account(
        mut,
        constraint = underlying_token_account.mint == underlying_mint.key() @ ErrorCode::TokenAccountMintMismatch,
        constraint = underlying_token_account.owner == creator.key() @ ErrorCode::InvalidTokenAccountOwner,
    )]
    pub underlying_token_account: Box<Account<'info, TokenAccount>>,
    #[account(
        mut,
        constraint = creator.key() == option_account.creator @ ErrorCode::OnlyCreatorCanBurnOptions
    )]
    pub creator: Signer<'info>,
    #[account(
        seeds = [OPTION_ACCOUNT_SEED.as_bytes().as_ref(), underlying.key().as_ref(), &option_account.underlying_count.to_le_bytes()],
        bump = option_account.option_account_nonce,
    )]
    pub option_account: Box<Account<'info, OptionAccount>>,
    #[account(
        seeds = [MINT_AUTH_SEED.as_bytes().as_ref()],
        bump = state.mint_auth_nonce,
    )]
    pub mint_authority: AccountInfo<'info>,
    #[account(
        mut,
        seeds = [OPTION_MINT_SEED.as_bytes().as_ref(), option_account.key().as_ref()],
        bump = option_account.option_mint_nonce,
    )]
    pub option_mint: Box<Account<'info, Mint>>,
    #[account(
        mut,
        seeds = [option_mint.key().as_ref(), creator.key().as_ref()],
        bump = option_account.creator_option_token_account_nonce,
        constraint = user_option_token_account.amount >= amount @ ErrorCode::InsufficientOptionsToBurn,
    )]
    pub user_option_token_account: Box<Account<'info, TokenAccount>>,
    pub token_program: Program<'info, Token>,
    #[account(
        seeds = [VAULT_AUTH_SEED.as_bytes().as_ref()],
        bump = state.vault_auth_nonce,
    )]
    pub vault_authority: AccountInfo<'info>,
}

#[derive(Accounts)]
pub struct CloseOptionAccount<'info> {
    pub state: Box<Account<'info, State>>,
    #[account(
        seeds = [UNDERLYING_SEED.as_bytes().as_ref(), underlying_mint.key().as_ref()],
        bump = underlying.underlying_nonce,
    )]
    pub underlying: Box<Account<'info, Underlying>>,
    #[account(
        mut,
        seeds = [VAULT_SEED.as_bytes().as_ref(), option_account.key().as_ref()],
        bump = option_account.vault_nonce,
    )]
    pub vault: Box<Account<'info, TokenAccount>>,
    pub underlying_mint: Box<Account<'info, Mint>>,
    #[account(
        mut,
        constraint = underlying_token_account.mint == underlying_mint.key() @ ErrorCode::TokenAccountMintMismatch,
        constraint = underlying_token_account.owner == creator.key() @ ErrorCode::InvalidTokenAccountOwner,
    )]
    pub underlying_token_account: Box<Account<'info, TokenAccount>>,
    #[account(
        mut,
        constraint = creator.key() == option_account.creator @ ErrorCode::OnlyCreatorCanCloseOptionAccount
    )]
    pub creator: Signer<'info>,
    #[account(
        mut,
        seeds = [OPTION_ACCOUNT_SEED.as_bytes().as_ref(), underlying.key().as_ref(), &option_account.underlying_count.to_le_bytes()],
        bump = option_account.option_account_nonce,
        close = creator,
    )]
    pub option_account: Box<Account<'info, OptionAccount>>,
    #[account(
        seeds = [MINT_AUTH_SEED.as_bytes().as_ref()],
        bump = state.mint_auth_nonce,
    )]
    pub mint_authority: AccountInfo<'info>,
    #[account(
        mut,
        seeds = [OPTION_MINT_SEED.as_bytes().as_ref(), option_account.key().as_ref()],
        bump = option_account.option_mint_nonce,
    )]
    pub option_mint: Box<Account<'info, Mint>>,
    #[account(
        mut,
        seeds = [option_mint.key().as_ref(), creator.key().as_ref()],
        bump = option_account.creator_option_token_account_nonce,
    )]
    pub user_option_token_account: Box<Account<'info, TokenAccount>>,
    pub token_program: Program<'info, Token>,
    #[account(
        seeds = [VAULT_AUTH_SEED.as_bytes().as_ref()],
        bump = state.vault_auth_nonce,
    )]
    pub vault_authority: AccountInfo<'info>,
}

#[derive(AnchorSerialize, AnchorDeserialize)]
pub struct InitializeOptionArgs {
    pub collateral_amount: u64,
    pub option_account_nonce: u8,
    pub option_mint_nonce: u8,
    pub token_account_nonce: u8,
    pub vault_nonce: u8,
    pub expiry: u64,
    pub strike: u64,
}

#[derive(AnchorSerialize, AnchorDeserialize)]
pub struct InitializeStateArgs {
    pub state_nonce: u8,
    pub mint_auth_nonce: u8,
    pub vault_auth_nonce: u8,
    pub settlement_price_threshold_seconds: u64,
}

#[derive(AnchorSerialize, AnchorDeserialize)]
pub struct InitializeUnderlyingArgs {
    pub underlying_nonce: u8,
}

#[account]
#[derive(Default)]
pub struct OptionAccount {
    pub option_account_nonce: u8,
    pub option_mint_nonce: u8,
    pub creator_option_token_account_nonce: u8,
    pub vault_nonce: u8,

    pub underlying_count: u64,
    pub option_mint: Pubkey,
    pub underlying_mint: Pubkey,
    pub creator: Pubkey,
    pub strike: u64,
    pub expiry: u64,
    pub settlement_price: u64,

    pub mint_supply_at_settlement: u64,
    pub profit_per_option: u64,
    pub remaining_collateral: u64,
}

#[account]
#[derive(Default)]
pub struct Underlying {
    pub underlying_nonce: u8,
    pub mint: Pubkey,
    pub oracle: Pubkey,
    pub count: u64,
}

#[account]
#[derive(Default)]
pub struct State {
    pub state_nonce: u8,
    pub mint_auth_nonce: u8,
    pub vault_auth_nonce: u8,
    pub admin: Pubkey,
    pub settlement_price_threshold_seconds: u64,
}

impl<'info> InitializeOption<'info> {
    pub fn into_mint_to_context(&self) -> CpiContext<'_, '_, '_, 'info, MintTo<'info>> {
        let cpi_accounts = MintTo {
            mint: self.option_mint.to_account_info().clone(),
            to: self.user_option_token_account.to_account_info().clone(),
            authority: self.mint_authority.clone(),
        };
        CpiContext::new(self.token_program.to_account_info().clone(), cpi_accounts)
    }

    pub fn into_transfer_context(&self) -> CpiContext<'_, '_, '_, 'info, Transfer<'info>> {
        let cpi_accounts = Transfer {
            from: self.underlying_token_account.to_account_info().clone(),
            to: self.vault.to_account_info().clone(),
            authority: self.creator.to_account_info().clone(),
        };
        CpiContext::new(self.token_program.to_account_info().clone(), cpi_accounts)
    }
}

impl<'info> BurnOption<'info> {
    pub fn into_burn_context(&self) -> CpiContext<'_, '_, '_, 'info, Burn<'info>> {
        let cpi_accounts = Burn {
            mint: self.option_mint.to_account_info().clone(),
            to: self.user_option_token_account.to_account_info().clone(),
            authority: self.creator.to_account_info().clone(),
        };
        CpiContext::new(self.token_program.to_account_info().clone(), cpi_accounts)
    }

    pub fn into_transfer_context(&self) -> CpiContext<'_, '_, '_, 'info, Transfer<'info>> {
        let cpi_accounts = Transfer {
            from: self.vault.to_account_info().clone(),
            to: self.underlying_token_account.to_account_info().clone(),
            authority: self.vault_authority.to_account_info().clone(),
        };
        CpiContext::new(self.token_program.to_account_info().clone(), cpi_accounts)
    }
}

impl<'info> CloseOptionAccount<'info> {
    pub fn into_transfer_context(&self) -> CpiContext<'_, '_, '_, 'info, Transfer<'info>> {
        let cpi_accounts = Transfer {
            from: self.vault.to_account_info().clone(),
            to: self.underlying_token_account.to_account_info().clone(),
            authority: self.vault_authority.to_account_info().clone(),
        };
        CpiContext::new(self.token_program.to_account_info().clone(), cpi_accounts)
    }

    pub fn into_close_vault_context(&self) -> CpiContext<'_, '_, '_, 'info, CloseAccount<'info>> {
        let cpi_accounts = CloseAccount {
            account: self.vault.to_account_info().clone(),
            destination: self.creator.to_account_info().clone(),
            authority: self.vault_authority.to_account_info().clone(),
        };
        CpiContext::new(self.token_program.to_account_info().clone(), cpi_accounts)
    }

    pub fn into_close_user_token_account(
        &self,
    ) -> CpiContext<'_, '_, '_, 'info, CloseAccount<'info>> {
        let cpi_accounts = CloseAccount {
            account: self.user_option_token_account.to_account_info().clone(),
            destination: self.creator.to_account_info().clone(),
            authority: self.creator.to_account_info().clone(),
        };
        CpiContext::new(self.token_program.to_account_info().clone(), cpi_accounts)
    }

    pub fn into_burn_context(&self) -> CpiContext<'_, '_, '_, 'info, Burn<'info>> {
        let cpi_accounts = Burn {
            mint: self.option_mint.to_account_info().clone(),
            to: self.user_option_token_account.to_account_info().clone(),
            authority: self.creator.to_account_info().clone(),
        };
        CpiContext::new(self.token_program.to_account_info().clone(), cpi_accounts)
    }
}

pub fn get_oracle_price(oracle: &AccountInfo) -> u128 {
    let oracle_price = pc::Price::load(&oracle).unwrap();
    (oracle_price.agg.price as u128)
        .checked_mul(10u128.pow(USDC_DECIMALS))
        .unwrap()
        .checked_div(10u128.pow((-oracle_price.expo) as u32))
        .unwrap() as u128
}

#[macro_export]
macro_rules! mint_authority {
    (bump = $bump:expr) => {
        &[MINT_AUTH_SEED.as_bytes().as_ref(), &[$bump]]
    };
}

#[macro_export]
macro_rules! vault_authority {
    (bump = $bump:expr) => {
        &[VAULT_AUTH_SEED.as_bytes().as_ref(), &[$bump]]
    };
}

pub fn get_token_amount_per_option(mint_account: &Mint) -> u64 {
    10u64
        .pow(mint_account.decimals.into())
        .checked_div(10u64.pow(OPTION_MINT_DECIMALS.into()))
        .unwrap()
}

#[error]
pub enum ErrorCode {
    #[msg("Unauthorized admin")]
    UnauthorizedAdmin,
    #[msg("Token account mint mismatch")]
    TokenAccountMintMismatch,
    #[msg("Invalid token account owner")]
    InvalidTokenAccountOwner,
    #[msg("Insufficient funds")]
    InsufficientFunds,
    #[msg("Insufficient options to burn")]
    InsufficientOptionsToBurn,
    #[msg("Only creator can close option account")]
    OnlyCreatorCanCloseOptionAccount,
    #[msg("Only creator can burn options")]
    OnlyCreatorCanBurnOptions,
    #[msg("Option expiration must be in the future")]
    OptionExpirationMustBeInTheFuture,
    #[msg("Not past option close time")]
    NotPastOptionCloseTime,
    #[msg("Only admin can override settlement price")]
    OnlyAdminCanOverrideSettlementPrice,
    #[msg("Before set settlement price time")]
    BeforeSetSettlementPriceTime,
    #[msg("After set settlement price time")]
    AfterSetSettlementPriceTime,
    #[msg("SettlementPriceAlreadySet")]
    SettlementPriceAlreadySet,
    #[msg("InvalidOracle")]
    InvalidOracle,
    #[msg("Invalid settlment option mint")]
    InvalidSettlementOptionMint,
    #[msg("Cannot burn options after expiry")]
    CannotBurnOptionsAfterExpiry,
    #[msg("Cannot burn options after settlement price is set")]
    CannotBurnOptionsAfterSettlementPriceIsSet,
}
