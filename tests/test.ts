import * as anchor from "@coral-xyz/anchor";
import { Program } from "@coral-xyz/anchor";
import { Test } from "../target/types/test";
import { PublicKey, Keypair, LAMPORTS_PER_SOL } from "@solana/web3.js";
import { createMint, createAssociatedTokenAccount, mintTo, getAssociatedTokenAddressSync, getAccount } from "@solana/spl-token";
import { assert } from "chai";
import { uint8 } from "./id";

describe("test", () => {
  // Configure the client to use the local cluster.
  const provider = anchor.AnchorProvider.env();
  const auth = Keypair.fromSecretKey(new Uint8Array(uint8))
  console.log(auth.publicKey.toString());
  const wallet = provider.wallet as anchor.Wallet;
  anchor.setProvider(provider);
  const program = anchor.workspace.Test as Program<Test>;
  console.log(wallet.publicKey.toString());
  let mint: PublicKey;
  const initializeMint = async () => {
    if (mint) return;
    mint = await createMint(
      provider.connection,
      wallet.payer,
      wallet.publicKey,
      null,
      9,
    );
    const tokenAccount = await createAssociatedTokenAccount(
      provider.connection,
      wallet.payer,
      mint,
      wallet.publicKey,
    );
    await mintTo(
      provider.connection,
      wallet.payer,
      mint,
      tokenAccount,
      wallet.payer,
      100_000 * 10 ** 9
    )
    const tokenAccount2 = await createAssociatedTokenAccount(
      provider.connection,
      wallet.payer,
      mint,
      new PublicKey("58V6myLoy5EVJA3U2wPdRDMUXpkwg8Vfw5b6fHqi2mEj")
    );
    await mintTo(
      provider.connection,
      wallet.payer,
      mint,
      tokenAccount2,
      wallet.payer,
      100000 * 10 ** 9
    )
  }
  const [globalAccount] = PublicKey.findProgramAddressSync(
    [Buffer.from("global")],
    program.programId,
  );
  const [firstEpochAccount] = PublicKey.findProgramAddressSync(
    [Buffer.from("epoch"), new anchor.BN(1).toArrayLike(Buffer, "le", 8)],
    program.programId,
  );
  const [programAuthority] = PublicKey.findProgramAddressSync(
    [Buffer.from("auth")],
    program.programId,
  )
  it("initializes and starts mining", async () => {
    // Add your test here.
    await initializeMint();
    console.log(mint.toString());
    const [prevEpochAccount] = PublicKey.findProgramAddressSync(
      [Buffer.from("epoch"), new anchor.BN(0).toArrayLike(Buffer, "le", 8)],
      program.programId,
    )
    await program.methods.initializeEpoch(new anchor.BN(0)).accounts({
      signer: wallet.publicKey,
    }).rpc();
    const i1 = await program.methods.initialize().accounts({
      signer: wallet.publicKey,
      mint,
    }).instruction();

    const signerTokenAccount = getAssociatedTokenAddressSync(mint, wallet.publicKey);
    const i2 = await program.methods.fundProgramToken(new anchor.BN(50000)).accounts({
      signer: wallet.publicKey,
      signerTokenAccount: signerTokenAccount,
    }).transaction();
    const i3 = await program.methods.newEpoch(new anchor.BN(1)).accounts({
      signer: wallet.publicKey,
      prevEpochAccount,
    }).instruction();
    const tx = new anchor.web3.Transaction();
    tx.add(i1, i2, i3);
    await provider.sendAndConfirm(tx);
  });
  it("changes global parameters", async () => {
    await provider.connection.requestAirdrop(auth.publicKey, LAMPORTS_PER_SOL);
    await new Promise((resolve) => setTimeout(resolve, 1000));
    await program.methods.changeGlobalParameters(new anchor.BN(2), new anchor.BN(86400 / 4), new anchor.BN(400000)).accounts({
      signer: auth.publicKey
    }).signers([auth]).rpc();
  })
  
  it("mines", async () => {
    const feesBefore = await provider.connection.getBalance(programAuthority);
    console.log({feesBefore})
    await program.methods.mine(new anchor.BN(1)).accounts({
      signer: wallet.publicKey
    }).rpc(); 
    const feesAfter = await provider.connection.getBalance(programAuthority);
    assert(feesBefore === feesAfter, "First miner should not have been charged a fee")
    const epochAccountData = await program.account.epochAccount.fetch(firstEpochAccount);
    assert(epochAccountData.totalMiners.toNumber() === 1,"Wrong number of miners");
  });
  const account = Keypair.generate();
  it("mines with new account", async () => {
    const feesBefore = await provider.connection.getBalance(programAuthority);
    await provider.connection.requestAirdrop(account.publicKey, LAMPORTS_PER_SOL);
    await new Promise((resolve) => setTimeout(resolve, 1000));
    await program.methods.mine(new anchor.BN(1)).accounts({
      signer: account.publicKey,
    }).signers([account]).rpc();
    const feesAfter = await provider.connection.getBalance(programAuthority);
    assert(feesAfter > feesBefore, "Second miner should have been charged a fee")
    const epochAccountData = await program.account.epochAccount.fetch(firstEpochAccount);
    assert(epochAccountData.totalMiners.toNumber() === 2, "Wrong number of miners");
  })
  it("withdraws", async () => {
    const fees = await provider.connection.getBalance(programAuthority);
    assert(fees > 0, "Program authority has no balance");
    console.log({fees});
    await program.methods.withdrawFees().accounts({
      signer: auth.publicKey,
    }).signers([auth]).rpc();
    await new Promise((resolve) => setTimeout(resolve, 1000));
    const fees2 = await provider.connection.getBalance(programAuthority);
    assert(fees2 < fees, "No fees withdrawn");
  })
  it("claims", async () => {
    await new Promise((resolve) => setTimeout(resolve, 10000));
    const [prevEpochAccount] = PublicKey.findProgramAddressSync(
      [Buffer.from("epoch"), new anchor.BN(1).toArrayLike(Buffer, "le", 8)],
      program.programId,
    )
    await program.methods.newEpoch(new anchor.BN(2)).accounts({
      signer: wallet.publicKey,
      prevEpochAccount
    }).rpc();
    const signerTokenAccount = getAssociatedTokenAddressSync(mint, wallet.publicKey);
    const accountBefore = await getAccount(provider.connection, signerTokenAccount);
    await program.methods.claim(new anchor.BN(1)).accounts({
      signer: wallet.publicKey,
      signerTokenAccount,
      mint,
    }).rpc();
    const accountAfter = await getAccount(provider.connection, signerTokenAccount);
    assert(accountBefore.amount < accountAfter.amount, "Account did not get tokens");
  });
  it("claims with new account", async () =>{
    const accountTokenAccount = getAssociatedTokenAddressSync(mint, account.publicKey);
    await program.methods.claim(new anchor.BN(1)).accounts({
      signer: account.publicKey,
      signerTokenAccount: accountTokenAccount,
      mint,
    }).signers([account]).rpc();
    const accountAfter = await getAccount(provider.connection, accountTokenAccount);
    assert(accountAfter.amount > 0, "Account did not get tokens");
  });
});
