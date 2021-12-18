import * as anchor from "@project-serum/anchor";
import { Program } from "@project-serum/anchor";
import { ZetaOtc } from "../target/types/zeta_otc";
import {
  PublicKey,
  Keypair,
  SystemProgram,
  SYSVAR_RENT_PUBKEY,
} from "@solana/web3.js";
import * as assert from "assert";
import * as utils from "./utils";
import { Token, TOKEN_PROGRAM_ID } from "@solana/spl-token";

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
  const admin = Keypair.generate();
  const tokenMintAuthority = Keypair.generate();
  const mintKeypair = Keypair.generate();
  const oracle = Keypair.generate();

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

  let collateralAmount = 1_000_000_000_000;
  let decimals = 9;
  let minLotSize = getMinLotSize(decimals);
  let expectedOptionTokenSupply = collateralAmount / minLotSize;
  // 10 seconds in future of creation.
  let expirationOffset = 5;
  let expirationTs: number;
  let settlementPriceThresholdSeconds = 5;

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
        oracle: oracle.publicKey,
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
    assert.ok(underlyingAccount.oracle.equals(oracle.publicKey));
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
    let strike = new anchor.BN(420);

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
  });

  it("Cannot close option account before close time.", async () => {
    await utils.expectError(async () => {
      await program.rpc.closeOptionAccount({
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
    }, "Not past option close time");
  });

  it("Close option account.", async () => {
    await utils.sleepTillTime(expirationTs);

    await program.rpc.closeOptionAccount({
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

    let userTokenAccount = await utils.getTokenAccountInfo(
      provider.connection,
      userTokenAddress
    );
    assert.ok(userTokenAccount.amount.toNumber() == collateralAmount);
  });
});
