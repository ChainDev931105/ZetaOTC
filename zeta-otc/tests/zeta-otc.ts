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

    let [_vault, vaultNonce] = await anchor.web3.PublicKey.findProgramAddress(
      [
        Buffer.from(anchor.utils.bytes.utf8.encode("vault")),
        token.publicKey.toBuffer(),
      ],
      program.programId
    );

    underlying = _underlying;
    vault = _vault;

    let args = {
      underlyingNonce,
      vaultNonce,
    };

    await program.rpc.initializeUnderlying(args, {
      accounts: {
        state,
        underlying,
        vault,
        mint: token.publicKey,
        oracle: oracle.publicKey,
        admin: admin.publicKey,
        vaultAuthority,
        systemProgram: SystemProgram.programId,
        tokenProgram: TOKEN_PROGRAM_ID,
        rent: SYSVAR_RENT_PUBKEY,
      },
      signers: [admin],
    });

    let underlyingAccount = await program.account.underlying.fetch(underlying);
    assert.ok(underlyingAccount.underlyingNonce == underlyingNonce);
    assert.ok(underlyingAccount.vaultNonce == vaultNonce);
    assert.ok(underlyingAccount.mint.equals(token.publicKey));
    assert.ok(underlyingAccount.oracle.equals(oracle.publicKey));
    assert.ok(underlyingAccount.count.eq(new anchor.BN(0)));
  });

  it("Initialize option", async () => {
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

    let [_optionMint, optionMintNonce] =
      await anchor.web3.PublicKey.findProgramAddress(
        [underlying.toBuffer(), count.toArrayLike(Buffer, "le", 8)],
        program.programId
      );

    optionAccount = _optionAccount;
    optionMint = _optionMint;

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
      expiry,
      strike,
    };

    await program.rpc.initializeOption(args, {
      accounts: {
        state,
        underlying,
        vault,
        underlyingMint: token.publicKey,
        underlyingTokenAccount: userTokenAddress,
        authority: provider.wallet.publicKey,
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
    console.log(optionAccountInfo);
    assert.ok(optionAccountInfo.creator.equals(provider.wallet.publicKey));
    assert.ok(optionAccountInfo.optionMint.equals(optionMint));
    assert.ok(optionAccountInfo.underlyingMint.equals(token.publicKey));
    assert.ok(optionAccountInfo.strike.eq(strike));
    assert.ok(optionAccountInfo.expiry.eq(expiry));

    let vaultInfo = await utils.getTokenAccountInfo(provider.connection, vault);
    assert.ok(vaultInfo.amount.toNumber() == collateralAmount);

    let userOptionTokenAccountInfo = await utils.getTokenAccountInfo(
      provider.connection,
      userOptionTokenAccount
    );
    assert.ok(userOptionTokenAccountInfo.amount.toNumber() == collateralAmount);
  });

  it("Burn options", async () => {
    let burnAmount = new anchor.BN(collateralAmount / 2);
    await program.rpc.burnOption(burnAmount, {
      accounts: {
        state,
        underlying,
        vault,
        underlyingMint: token.publicKey,
        underlyingTokenAccount: userTokenAddress,
        authority: provider.wallet.publicKey,
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
      userOptionTokenAccountInfo.amount.toNumber() == collateralAmount / 2
    );

    let userTokenAccount = await utils.getTokenAccountInfo(
      provider.connection,
      userTokenAddress
    );
    assert.ok(userTokenAccount.amount.toNumber() == collateralAmount / 2);
  });
});
