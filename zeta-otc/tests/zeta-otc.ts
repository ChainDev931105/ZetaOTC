import * as anchor from "@project-serum/anchor";
import { Program } from "@project-serum/anchor";
import { ZetaOtc } from "../target/types/zeta_otc";
import { Pyth } from "../target/types/pyth";
import {
  PublicKey,
  Keypair,
  SystemProgram,
  SYSVAR_RENT_PUBKEY,
} from "@solana/web3.js";
import * as assert from "assert";
import * as utils from "./utils";
import { Token, TOKEN_PROGRAM_ID } from "@solana/spl-token";
import { createPriceFeed } from "./oracle-utils";

const OPTION_MINT_DECIMALS: number = 4;

function getMinLotSize(mintDecimals: number): number {
  if (mintDecimals < OPTION_MINT_DECIMALS) {
    throw Error("");
  }
  return 10 ** mintDecimals / 10 ** OPTION_MINT_DECIMALS;
}

describe("zeta-otc", () => {
  // Configure the client to use the local cluster.
  let provider = anchor.Provider.env();
  anchor.setProvider(provider);

  const program = anchor.workspace.ZetaOtc as Program<ZetaOtc>;
  const pythProgram = anchor.workspace.Pyth as Program<Pyth>;
  const admin = Keypair.generate();
  const tokenMintAuthority = Keypair.generate();
  const mintKeypair = Keypair.generate();
  const otherUser = Keypair.generate();
  let otherUserOptionAccount: PublicKey;
  let otherUserTokenAddress: PublicKey;

  let state: PublicKey;
  let mintAuthority: PublicKey;
  let vaultAuthority: PublicKey;
  let underlying: PublicKey;
  let token: Token;
  let userTokenAddress: PublicKey;
  let vault: PublicKey;
  let optionAccount: PublicKey;
  let optionMint: PublicKey;
  let userOptionTokenAccount: PublicKey;
  let oracle: PublicKey;

  let collateralAmount = 1_000_000_000_000;
  let decimals = 9;
  let minLotSize = getMinLotSize(decimals);
  let expectedOptionTokenSupply = collateralAmount / minLotSize;
  // 10 seconds in future of creation.
  let expirationOffset = 5;
  let expirationTs: number;
  let settlementPriceThresholdSeconds = 5;
  let oraclePrice = 175;
  let nativeOraclePrice = oraclePrice * 10 ** 6;
  let strike = new anchor.BN(150_000_000); // 150

  it("Create oracle price feed.", async () => {
    oracle = await createPriceFeed({
      oracleProgram: pythProgram,
      initPrice: oraclePrice,
      confidence: 0.1,
      keypair: utils.getOracleKeypair(),
      expo: -8,
    });
  });

  it("Create mint and mint to user.", async () => {
    token = await utils.createMint(
      provider.connection,
      mintKeypair,
      (provider.wallet as anchor.Wallet).payer,
      tokenMintAuthority.publicKey,
      decimals
    );

    userTokenAddress = await token.createAssociatedTokenAccount(
      provider.wallet.publicKey
    );

    await token.mintTo(
      userTokenAddress,
      tokenMintAuthority.publicKey,
      [tokenMintAuthority],
      collateralAmount
    );
  });

  it("Initialize state", async () => {
    await program.provider.connection.confirmTransaction(
      await program.provider.connection.requestAirdrop(
        admin.publicKey,
        10000000000
      ),
      "confirmed"
    );

    let [_state, stateNonce] = await anchor.web3.PublicKey.findProgramAddress(
      [Buffer.from(anchor.utils.bytes.utf8.encode("state"))],
      program.programId
    );

    let [_mintAuthority, mintAuthNonce] =
      await anchor.web3.PublicKey.findProgramAddress(
        [Buffer.from(anchor.utils.bytes.utf8.encode("mint-auth"))],
        program.programId
      );

    let [_vaultAuthority, vaultAuthNonce] =
      await anchor.web3.PublicKey.findProgramAddress(
        [Buffer.from(anchor.utils.bytes.utf8.encode("vault-auth"))],
        program.programId
      );

    state = _state;
    mintAuthority = _mintAuthority;
    vaultAuthority = _vaultAuthority;

    let args = {
      stateNonce,
      mintAuthNonce,
      vaultAuthNonce,
      settlementPriceThresholdSeconds,
    };

    await program.rpc.initializeState(args, {
      accounts: {
        state,
        systemProgram: SystemProgram.programId,
        admin: admin.publicKey,
        mintAuthority,
        vaultAuthority,
      },
      signers: [admin],
    });

    let stateAccount = await program.account.state.fetch(state);
    assert.ok(stateAccount.admin.equals(admin.publicKey));
    assert.ok(stateAccount.stateNonce == stateNonce);
    assert.ok(stateAccount.vaultAuthNonce == vaultAuthNonce);
    assert.ok(stateAccount.mintAuthNonce == mintAuthNonce);
  });

  it("Initialize underlying", async () => {
    let [_underlying, underlyingNonce] =
      await anchor.web3.PublicKey.findProgramAddress(
        [
          Buffer.from(anchor.utils.bytes.utf8.encode("underlying")),
          token.publicKey.toBuffer(),
        ],
        program.programId
      );

    underlying = _underlying;

    let args = {
      underlyingNonce,
    };

    await program.rpc.initializeUnderlying(args, {
      accounts: {
        state,
        underlying,
        mint: token.publicKey,
        oracle: oracle,
        admin: admin.publicKey,
        systemProgram: SystemProgram.programId,
        tokenProgram: TOKEN_PROGRAM_ID,
        rent: SYSVAR_RENT_PUBKEY,
      },
      signers: [admin],
    });

    let underlyingAccount = await program.account.underlying.fetch(underlying);
    assert.ok(underlyingAccount.underlyingNonce == underlyingNonce);
    assert.ok(underlyingAccount.mint.equals(token.publicKey));
    assert.ok(underlyingAccount.oracle.equals(oracle));
    assert.ok(underlyingAccount.count.eq(new anchor.BN(0)));
  });

  it("Initialize option", async () => {
    let now = Date.now() / 1000;
    expirationTs = now + expirationOffset;

    let count = new anchor.BN(0);
    let [_optionAccount, optionAccountNonce] =
      await anchor.web3.PublicKey.findProgramAddress(
        [
          Buffer.from(anchor.utils.bytes.utf8.encode("option-account")),
          underlying.toBuffer(),
          count.toArrayLike(Buffer, "le", 8),
        ],
        program.programId
      );

    optionAccount = _optionAccount;

    let [_vault, vaultNonce] = await anchor.web3.PublicKey.findProgramAddress(
      [
        Buffer.from(anchor.utils.bytes.utf8.encode("vault")),
        optionAccount.toBuffer(),
      ],
      program.programId
    );

    let [_optionMint, optionMintNonce] =
      await anchor.web3.PublicKey.findProgramAddress(
        [
          Buffer.from(anchor.utils.bytes.utf8.encode("option-mint")),
          optionAccount.toBuffer(),
        ],
        program.programId
      );

    optionMint = _optionMint;
    vault = _vault;

    let [_userOptionTokenAccount, userOptionTokenAccountNonce] =
      await anchor.web3.PublicKey.findProgramAddress(
        [optionMint.toBuffer(), provider.wallet.publicKey.toBuffer()],
        program.programId
      );
    userOptionTokenAccount = _userOptionTokenAccount;

    let expiry = new anchor.BN(123);

    let args = {
      collateralAmount: new anchor.BN(collateralAmount),
      optionAccountNonce,
      optionMintNonce,
      tokenAccountNonce: userOptionTokenAccountNonce,
      vaultNonce,
      expiry,
      strike,
    };

    await utils.expectError(async () => {
      await program.rpc.initializeOption(args, {
        accounts: {
          state,
          underlying,
          vault,
          vaultAuthority,
          underlyingMint: token.publicKey,
          underlyingTokenAccount: userTokenAddress,
          creator: provider.wallet.publicKey,
          optionAccount,
          mintAuthority,
          optionMint,
          userOptionTokenAccount,
          systemProgram: SystemProgram.programId,
          tokenProgram: TOKEN_PROGRAM_ID,
          rent: SYSVAR_RENT_PUBKEY,
        },
      });
    }, "Option expiration must be in the future");

    args.expiry = new anchor.BN(expirationTs);

    await program.rpc.initializeOption(args, {
      accounts: {
        state,
        underlying,
        vault,
        vaultAuthority,
        underlyingMint: token.publicKey,
        underlyingTokenAccount: userTokenAddress,
        creator: provider.wallet.publicKey,
        optionAccount,
        mintAuthority,
        optionMint,
        userOptionTokenAccount,
        systemProgram: SystemProgram.programId,
        tokenProgram: TOKEN_PROGRAM_ID,
        rent: SYSVAR_RENT_PUBKEY,
      },
    });

    let optionAccountInfo = await program.account.optionAccount.fetch(
      optionAccount
    );
    assert.ok(optionAccountInfo.creator.equals(provider.wallet.publicKey));
    assert.ok(optionAccountInfo.optionMint.equals(optionMint));
    assert.ok(optionAccountInfo.underlyingMint.equals(token.publicKey));
    assert.ok(optionAccountInfo.strike.eq(strike));
    assert.ok(optionAccountInfo.expiry.eq(args.expiry));
    assert.ok(optionAccountInfo.optionAccountNonce == optionAccountNonce);
    assert.ok(optionAccountInfo.optionMintNonce == optionMintNonce);
    assert.ok(optionAccountInfo.vaultNonce == vaultNonce);
    assert.ok(
      optionAccountInfo.creatorOptionTokenAccountNonce ==
        userOptionTokenAccountNonce
    );
    assert.ok(optionAccountInfo.remainingCollateral.toNumber() == 0);

    let vaultInfo = await utils.getTokenAccountInfo(provider.connection, vault);
    assert.ok(vaultInfo.amount.toNumber() == collateralAmount);

    let userOptionTokenAccountInfo = await utils.getTokenAccountInfo(
      provider.connection,
      userOptionTokenAccount
    );
    assert.ok(
      userOptionTokenAccountInfo.amount.toNumber() == expectedOptionTokenSupply
    );
    let mintInfo = await utils.getMintInfo(provider.connection, optionMint);
    assert.ok(mintInfo.decimals == OPTION_MINT_DECIMALS);
    assert.ok(mintInfo.supply.toNumber() == expectedOptionTokenSupply);
    assert.ok(mintInfo.mintAuthority.equals(mintAuthority));

    let underlyingAccount = await program.account.underlying.fetch(underlying);
    assert.ok(underlyingAccount.count.eq(new anchor.BN(1)));
  });

  it("Burn options", async () => {
    let burnAmount = new anchor.BN(expectedOptionTokenSupply / 2);
    await program.rpc.burnOption(burnAmount, {
      accounts: {
        state,
        underlying,
        vault,
        underlyingMint: token.publicKey,
        underlyingTokenAccount: userTokenAddress,
        creator: provider.wallet.publicKey,
        optionAccount,
        mintAuthority,
        optionMint,
        userOptionTokenAccount,
        tokenProgram: TOKEN_PROGRAM_ID,
        vaultAuthority,
      },
    });

    let userOptionTokenAccountInfo = await utils.getTokenAccountInfo(
      provider.connection,
      userOptionTokenAccount
    );

    assert.ok(
      userOptionTokenAccountInfo.amount.toNumber() ==
        expectedOptionTokenSupply / 2
    );

    let userTokenAccount = await utils.getTokenAccountInfo(
      provider.connection,
      userTokenAddress
    );
    assert.ok(userTokenAccount.amount.toNumber() == collateralAmount / 2);

    console.log("Remaining collateral = ", collateralAmount / 2);
    console.log("Remaining token supply = ", expectedOptionTokenSupply / 2);
  });

  let profitPerOption: number;
  let transferAmount: number = 10000;

  it("Expire option.", async () => {
    await utils.sleepTillTime(expirationTs);

    await program.rpc.expireOption({
      accounts: {
        state,
        underlying,
        underlyingMint: token.publicKey,
        optionAccount,
        oracle,
        optionMint,
        vault,
      },
    });

    let optionAccountInfo = await program.account.optionAccount.fetch(
      optionAccount
    );

    let vaultInfo = await utils.getTokenAccountInfo(provider.connection, vault);
    let mintInfo = await utils.getMintInfo(provider.connection, optionMint);
    let underlyingAmount = vaultInfo.amount.toNumber();
    let remainingOptionSupply = mintInfo.supply.toNumber();
    let itmAmount = Math.max(0, nativeOraclePrice - strike.toNumber());
    let tokenAmountPerOption = getMinLotSize(decimals);
    profitPerOption = Math.floor(
      (tokenAmountPerOption * itmAmount) / nativeOraclePrice
    );
    let totalProfit = profitPerOption * remainingOptionSupply;
    let remainingCollateral = underlyingAmount - totalProfit;

    console.log(`Profit per option : ${profitPerOption}`);
    console.log(`Remaining collateral: ${underlyingAmount - totalProfit}`);
    console.log(`${optionAccountInfo.profitPerOption.toNumber()}`);
    console.log(`${optionAccountInfo.remainingCollateral.toNumber()}`);
    assert.ok(profitPerOption == optionAccountInfo.profitPerOption.toNumber());
    assert.ok(
      remainingCollateral == optionAccountInfo.remainingCollateral.toNumber()
    );
    assert.ok(
      optionAccountInfo.settlementPrice.toNumber() == nativeOraclePrice
    );
  });

  it("Expire option override.", async () => {
    let overrideSettlementPrice = new anchor.BN(200 * Math.pow(10, 6));

    await program.rpc.expireOptionOverride(overrideSettlementPrice, {
      accounts: {
        state,
        underlying,
        underlyingMint: token.publicKey,
        optionAccount,
        optionMint,
        vault,
        admin: admin.publicKey,
      },
      signers: [admin],
    });

    let optionAccountInfo = await program.account.optionAccount.fetch(
      optionAccount
    );

    let vaultInfo = await utils.getTokenAccountInfo(provider.connection, vault);
    let mintInfo = await utils.getMintInfo(provider.connection, optionMint);
    let underlyingAmount = vaultInfo.amount.toNumber();
    let remainingOptionSupply = mintInfo.supply.toNumber();
    let itmAmount = Math.max(
      0,
      overrideSettlementPrice.toNumber() - strike.toNumber()
    );
    let tokenAmountPerOption = getMinLotSize(decimals);
    profitPerOption =
      (tokenAmountPerOption * itmAmount) / overrideSettlementPrice.toNumber();
    let totalProfit = profitPerOption * remainingOptionSupply;
    let remainingCollateral = underlyingAmount - totalProfit;

    console.log(`Profit per option : ${profitPerOption}`);
    console.log(`Remaining collateral: ${underlyingAmount - totalProfit}`);
    assert.ok(profitPerOption == optionAccountInfo.profitPerOption.toNumber());
    assert.ok(
      remainingCollateral == optionAccountInfo.remainingCollateral.toNumber()
    );
  });

  it("Transfer token to other user", async () => {
    let optionToken = new Token(
      provider.connection,
      optionMint,
      TOKEN_PROGRAM_ID,
      (provider.wallet as anchor.Wallet).payer
    );

    otherUserOptionAccount = await optionToken.createAccount(
      otherUser.publicKey
    );

    await optionToken.transfer(
      userOptionTokenAccount,
      otherUserOptionAccount,
      provider.wallet.publicKey,
      [(provider.wallet as anchor.Wallet).payer],
      transferAmount
    );

    otherUserTokenAddress = await token.createAssociatedTokenAccount(
      otherUser.publicKey
    );
  });

  it("Other user exercise option.", async () => {
    await program.rpc.exerciseOption({
      accounts: {
        state,
        underlying,
        vault,
        underlyingMint: token.publicKey,
        underlyingTokenAccount: otherUserTokenAddress,
        authority: otherUser.publicKey,
        optionAccount,
        optionMint,
        userOptionTokenAccount: otherUserOptionAccount,
        tokenProgram: TOKEN_PROGRAM_ID,
        vaultAuthority,
      },
      signers: [otherUser],
    });

    let expectedTokenBalance = profitPerOption * transferAmount;
    console.log(`Expected token balance : ${expectedTokenBalance}`);

    let userTokenAccountInfo = await utils.getTokenAccountInfo(
      provider.connection,
      otherUserTokenAddress
    );
    let userOptionAccountInfo = await utils.getTokenAccountInfo(
      provider.connection,
      otherUserOptionAccount
    );
    assert.ok(userTokenAccountInfo.amount.toNumber() == expectedTokenBalance);
    assert.ok(userOptionAccountInfo.amount.toNumber() == 0);
  });

  it("Owner exercises option.", async () => {
    let userTokenAccount = await utils.getTokenAccountInfo(
      provider.connection,
      userTokenAddress
    );
    let userOptionTokenAccountInfo = await utils.getTokenAccountInfo(
      provider.connection,
      userOptionTokenAccount
    );
    let prevTokenBalance = userTokenAccount.amount.toNumber();
    let optionBalance = userOptionTokenAccountInfo.amount.toNumber();
    let expectedTokenBalanceDiff = optionBalance * profitPerOption;

    await program.rpc.exerciseOption({
      accounts: {
        state,
        underlying,
        vault,
        underlyingMint: token.publicKey,
        underlyingTokenAccount: userTokenAddress,
        authority: provider.wallet.publicKey,
        optionAccount,
        optionMint,
        userOptionTokenAccount,
        tokenProgram: TOKEN_PROGRAM_ID,
        vaultAuthority,
      },
    });

    userTokenAccount = await utils.getTokenAccountInfo(
      provider.connection,
      userTokenAddress
    );

    let totalProfit = userTokenAccount.amount.toNumber() - prevTokenBalance;
    assert.ok(totalProfit == expectedTokenBalanceDiff);

    userOptionTokenAccountInfo = await utils.getTokenAccountInfo(
      provider.connection,
      userOptionTokenAccount
    );
    assert.ok(userOptionTokenAccountInfo.amount.toNumber() == 0);
  });

  it("Owner collects remaining collateral", async () => {
    await program.rpc.collectRemainingCollateral({
      accounts: {
        state,
        underlying,
        vault,
        underlyingMint: token.publicKey,
        underlyingTokenAccount: userTokenAddress,
        creator: provider.wallet.publicKey,
        optionAccount,
        optionMint,
        userOptionTokenAccount,
        tokenProgram: TOKEN_PROGRAM_ID,
        vaultAuthority,
      },
    });

    let vaultInfo = await utils.getTokenAccountInfo(provider.connection, vault);
    assert.ok(vaultInfo.amount.toNumber() == 0);

    let optionAccountInfo = await program.account.optionAccount.fetch(
      optionAccount
    );
    assert.ok(optionAccountInfo.remainingCollateral.toNumber() == 0);
    try {
      await utils.getTokenAccountInfo(
        provider.connection,
        userOptionTokenAccount
      );
    } catch (e) {}

    let userTokenAccount = await utils.getTokenAccountInfo(
      provider.connection,
      userTokenAddress
    );
    assert.ok(
      userTokenAccount.amount.toNumber() ==
        collateralAmount - profitPerOption * transferAmount
    );
  });
});
