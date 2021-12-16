use anchor_lang::prelude::*;
use anchor_spl::token::{self, Burn, Mint, MintTo, Token, TokenAccount, Transfer};

declare_id!("Fg6PaFpoGXkYsidMpWTK6W2BeZ7FEfcYkg476zPFsLnS");

pub const UNDERLYING_SEED: &str = "underlying";
pub const STATE_SEED: &str = "state";
pub const MINT_AUTH_SEED: &str = "mint-auth";
pub const VAULT_AUTH_SEED: &str = "vault-auth";
pub const VAULT_SEED: &str = "vault";
pub const OPTION_ACCOUNT_SEED: &str = "option-account";

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
        ctx.accounts.underlying.vault_nonce = args.vault_nonce;
        ctx.accounts.underlying.mint = ctx.accounts.mint.key();
        ctx.accounts.underlying.oracle = ctx.accounts.oracle.key();
        Ok(())
    }

    pub fn initialize_option(
        ctx: Context<InitializeOption>,
        args: InitializeOptionArgs,
    ) -> ProgramResult {
        let option_account = &mut ctx.accounts.option_account;
        option_account.option_account_nonce = args.option_account_nonce;
        option_account.option_mint_nonce = args.option_mint_nonce;
        option_account.creator_option_token_account_nonce = args.token_account_nonce;
        option_account.option_mint = ctx.accounts.option_mint.key();
        option_account.underlying_mint = ctx.accounts.underlying_mint.key();
        option_account.creator = ctx.accounts.authority.key();
        option_account.strike = args.strike;
        option_account.expiry = args.expiry;

        let mint_seeds = mint_authority! {
            bump = ctx.accounts.state.mint_auth_nonce
        };

        token::mint_to(
            ctx.accounts
                .into_mint_to_context()
                .with_signer(&[&mint_seeds[..]]),
            // TODO this should be divided by mint.decimals
            args.collateral_amount,
        )?;

        token::transfer(ctx.accounts.into_transfer_context(), args.collateral_amount)?;

        Ok(())
    }

    pub fn burn_option(ctx: Context<BurnOption>, amount: u64) -> ProgramResult {
        let mint_seeds = mint_authority! {
            bump = ctx.accounts.state.mint_auth_nonce
        };

        let vault_seeds = vault_authority! {
            bump = ctx.accounts.state.vault_auth_nonce
        };

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
            amount,
        )?;

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
    #[account(
        init,
        token::mint = mint,
        token::authority = vault_authority,
        seeds = [VAULT_SEED.as_bytes().as_ref(), mint.key().as_ref()],
        bump = args.vault_nonce,
        payer = admin,
    )]
    pub vault: Account<'info, TokenAccount>,
    pub mint: Account<'info, Mint>,
    pub oracle: UncheckedAccount<'info>,
    #[account(mut)]
    pub admin: Signer<'info>,
    #[account(
        seeds = [VAULT_AUTH_SEED.as_bytes().as_ref()],
        bump = state.vault_auth_nonce,
    )]
    pub vault_authority: AccountInfo<'info>,
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
        mut,
        seeds = [VAULT_SEED.as_bytes().as_ref(), underlying_mint.key().as_ref()],
        bump = underlying.vault_nonce,
    )]
    pub vault: Box<Account<'info, TokenAccount>>,
    pub underlying_mint: Box<Account<'info, Mint>>,
    #[account(
        mut,
        constraint = underlying_token_account.mint == underlying_mint.key() @ ErrorCode::TokenAccountMintMismatch,
        constraint = underlying_token_account.owner == authority.key() @ ErrorCode::InvalidTokenAccountOwner,
        constraint = underlying_token_account.amount >= args.collateral_amount @ ErrorCode::InsufficientFunds,
    )]
    pub underlying_token_account: Box<Account<'info, TokenAccount>>,
    #[account(mut)]
    pub authority: Signer<'info>,
    #[account(
        init,
        seeds = [OPTION_ACCOUNT_SEED.as_bytes().as_ref(), underlying.key().as_ref(), &underlying.count.to_le_bytes()],
        bump = args.option_account_nonce,
        payer = authority,
    )]
    pub option_account: Box<Account<'info, OptionAccount>>,
    #[account(
        seeds = [MINT_AUTH_SEED.as_bytes().as_ref()],
        bump = state.mint_auth_nonce,
    )]
    pub mint_authority: AccountInfo<'info>,
    #[account(
        init,
        mint::decimals = 0,
        mint::authority = mint_authority,
        seeds = [underlying.key().as_ref(), &underlying.count.to_le_bytes()],
        bump = args.option_mint_nonce,
        payer = authority,
    )]
    pub option_mint: Box<Account<'info, Mint>>,
    #[account(
        init,
        token::mint = option_mint,
        token::authority = authority,
        seeds = [option_mint.key().as_ref(), authority.key().as_ref()],
        bump = args.token_account_nonce,
        payer = authority,
    )]
    pub user_option_token_account: Box<Account<'info, TokenAccount>>,
    pub system_program: Program<'info, System>,
    pub token_program: Program<'info, Token>,
    pub rent: Sysvar<'info, Rent>,
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
        seeds = [VAULT_SEED.as_bytes().as_ref(), underlying_mint.key().as_ref()],
        bump = underlying.vault_nonce,
    )]
    pub vault: Box<Account<'info, TokenAccount>>,
    pub underlying_mint: Box<Account<'info, Mint>>,
    #[account(
        mut,
        constraint = underlying_token_account.mint == underlying_mint.key() @ ErrorCode::TokenAccountMintMismatch,
        constraint = underlying_token_account.owner == authority.key() @ ErrorCode::InvalidTokenAccountOwner,
    )]
    pub underlying_token_account: Box<Account<'info, TokenAccount>>,
    #[account(
        mut,
        constraint = authority.key() == option_account.creator @ ErrorCode::OnlyCreatorCanBurnOptions
    )]
    pub authority: Signer<'info>,
    #[account(
        seeds = [OPTION_ACCOUNT_SEED.as_bytes().as_ref(), underlying.key().as_ref(), &underlying.count.to_le_bytes()],
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
        seeds = [underlying.key().as_ref(), &underlying.count.to_le_bytes()],
        bump = option_account.option_mint_nonce,
    )]
    pub option_mint: Box<Account<'info, Mint>>,
    #[account(
        mut,
        seeds = [option_mint.key().as_ref(), authority.key().as_ref()],
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

#[derive(AnchorSerialize, AnchorDeserialize)]
pub struct InitializeOptionArgs {
    pub collateral_amount: u64,
    pub option_account_nonce: u8,
    pub option_mint_nonce: u8,
    pub token_account_nonce: u8,
    pub expiry: u64,
    pub strike: u64,
}

#[derive(AnchorSerialize, AnchorDeserialize)]
pub struct InitializeStateArgs {
    pub state_nonce: u8,
    pub mint_auth_nonce: u8,
    pub vault_auth_nonce: u8,
}

#[derive(AnchorSerialize, AnchorDeserialize)]
pub struct InitializeUnderlyingArgs {
    pub underlying_nonce: u8,
    pub vault_nonce: u8,
}

#[account]
#[derive(Default)]
pub struct OptionAccount {
    pub option_account_nonce: u8,
    pub option_mint_nonce: u8,
    pub creator_option_token_account_nonce: u8,
    pub option_mint: Pubkey,
    pub underlying_mint: Pubkey,
    pub creator: Pubkey,
    pub strike: u64,
    pub expiry: u64,
    pub settlement_price: u64,
}

#[account]
#[derive(Default)]
pub struct Underlying {
    pub underlying_nonce: u8,
    pub vault_nonce: u8,
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
            authority: self.authority.to_account_info().clone(),
        };
        CpiContext::new(self.token_program.to_account_info().clone(), cpi_accounts)
    }
}

impl<'info> BurnOption<'info> {
    pub fn into_burn_context(&self) -> CpiContext<'_, '_, '_, 'info, Burn<'info>> {
        let cpi_accounts = Burn {
            mint: self.option_mint.to_account_info().clone(),
            to: self.user_option_token_account.to_account_info().clone(),
            authority: self.authority.to_account_info().clone(),
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
    #[msg("Only creator can burn options")]
    OnlyCreatorCanBurnOptions,
}
