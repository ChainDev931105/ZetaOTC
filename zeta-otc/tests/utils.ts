import { Token } from "@solana/spl-token";
import {
  AccountLayout as TokenAccountLayout,
  MintLayout,
  TOKEN_PROGRAM_ID,
  u64,
} from "@solana/spl-token";
import {
  ConfirmOptions,
  SystemProgram,
  Connection,
  PublicKey,
  Keypair,
  Transaction,
  TransactionInstruction,
  sendAndConfirmTransaction,
  SYSVAR_RENT_PUBKEY,
} from "@solana/web3.js";
import BufferLayout from "buffer-layout";

export async function createMint(
  connection: Connection,
  mintKeypair: Keypair,
  payer: Keypair,
  mintAuthority: PublicKey,
  decimals: number
): Promise<Token> {
  const token = new Token(
    connection,
    mintKeypair.publicKey,
    TOKEN_PROGRAM_ID,
    payer
  ); // Allocate memory for the account
  const balanceNeeded = await Token.getMinBalanceRentForExemptMint(connection);
  const tx = new Transaction();
  tx.add(
    SystemProgram.createAccount({
      fromPubkey: payer.publicKey,
      newAccountPubkey: mintKeypair.publicKey,
      lamports: balanceNeeded,
      space: MintLayout.span,
      programId: TOKEN_PROGRAM_ID,
    })
  );
  tx.add(
    createInitMintInstruction(mintKeypair.publicKey, decimals, mintAuthority)
  );
  // Send the two instructions
  await sendAndConfirmTransaction(
    connection,
    tx,
    [payer, mintKeypair],
    defaultCommitment()
  );

  function createInitMintInstruction(
    mint: PublicKey,
    decimals: number,
    mintAuthority: PublicKey
  ): TransactionInstruction {
    let keys = [
      { pubkey: mint, isSigner: false, isWritable: true },
      { pubkey: SYSVAR_RENT_PUBKEY, isSigner: false, isWritable: false },
    ];
    const commandDataLayout = BufferLayout.struct([
      BufferLayout.u8("instruction"),
      BufferLayout.u8("decimals"),
      publicKey("mintAuthority"),
      BufferLayout.u8("option"),
      publicKey("freezeAuthority"),
    ]);
    let data = Buffer.alloc(1024);
    {
      const encodeLength = commandDataLayout.encode(
        {
          instruction: 0, // InitializeMint instruction
          decimals,
          mintAuthority: mintAuthority.toBuffer(),
          option: 0,
          freezeAuthority: new PublicKey(0).toBuffer(),
        },
        data
      );
      data = data.slice(0, encodeLength);
    }

    return new TransactionInstruction({
      keys,
      programId: TOKEN_PROGRAM_ID,
      data,
    });
  }
  return token;
}

export const publicKey = (property: string = "publicKey"): Object => {
  return BufferLayout.blob(32, property);
};

export function defaultCommitment(): ConfirmOptions {
  return {
    skipPreflight: false,
    preflightCommitment: "processed",
    commitment: "processed",
  };
}

export async function getTokenAccountInfo(
  connection: Connection,
  key: PublicKey
): Promise<any> {
  let info = await connection.getAccountInfo(key);
  if (info === null) {
    throw Error(`Token account ${key.toString()} doesn't exist.`);
  }
  if (info.data.length != TokenAccountLayout.span) {
    throw new Error(`Invalid account size`);
  }

  const data = Buffer.from(info.data);
  const accountInfo = TokenAccountLayout.decode(data);
  accountInfo.address = key;
  accountInfo.mint = new PublicKey(accountInfo.mint);
  accountInfo.owner = new PublicKey(accountInfo.owner);
  accountInfo.amount = u64.fromBuffer(accountInfo.amount);

  if (accountInfo.delegateOption === 0) {
    accountInfo.delegate = null;
    accountInfo.delegatedAmount = new u64(0);
  } else {
    accountInfo.delegate = new PublicKey(accountInfo.delegate);
    accountInfo.delegatedAmount = u64.fromBuffer(accountInfo.delegatedAmount);
  }

  accountInfo.isInitialized = accountInfo.state !== 0;
  accountInfo.isFrozen = accountInfo.state === 2;

  if (accountInfo.isNativeOption === 1) {
    accountInfo.rentExemptReserve = u64.fromBuffer(accountInfo.isNative);
    accountInfo.isNative = true;
  } else {
    accountInfo.rentExemptReserve = null;
    accountInfo.isNative = false;
  }

  if (accountInfo.closeAuthorityOption === 0) {
    accountInfo.closeAuthority = null;
  } else {
    accountInfo.closeAuthority = new PublicKey(accountInfo.closeAuthority);
  }

  return accountInfo;
}
