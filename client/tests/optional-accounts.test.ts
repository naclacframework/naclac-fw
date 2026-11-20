/**
 * @naclac/client — leaving an optional account out. Passing `null` for an
 * optional account fills its slot with the program's own address, exactly as
 * the generated Rust client does for `None`; every other case is an error.
 */

import { strict as assert } from "assert";
import { Keypair } from "@solana/web3.js";
import { AccountRole, generateKeyPairSigner } from "@solana/kit";

import { LegacyMethodsBuilder } from "../src/legacy";
import { MethodsBuilder } from "../src/kit/methods";
import type { NaclacIdl } from "../src/idl";

const PROGRAM_ID = "Fg6PaFpoGXkYsidMpWTK6W2BeZ7FEfcYkg476zPFsLnS";

const idl: NaclacIdl = {
  address: PROGRAM_ID,
  metadata: { name: "optional_lab", version: "0.1.0", spec: "0.1.0" },
  instructions: [
    {
      name: "touch",
      discriminator: [1, 26],
      accounts: [
        { name: "signer", signer: true },
        { name: "note", writable: true, optional: true },
        { name: "vault", writable: true },
      ],
      args: [],
    },
  ],
  accounts: [],
  events: [],
  errors: [],
};

const provider = {} as any;

describe("optional accounts: kit builder", () => {
  const builder = () => new MethodsBuilder(idl, idl.instructions[0], {}, provider);

  it("fills a null optional account with the program id and keeps its writable role", async () => {
    const signer = await generateKeyPairSigner();
    const vault = await generateKeyPairSigner();
    const ix = await builder()
      .accounts({ signer: signer.address, note: null, vault: vault.address })
      .instruction();
    const accounts = ix.accounts!;
    assert.equal(accounts.length, 3);
    assert.equal(accounts[1].address, PROGRAM_ID);
    assert.equal(accounts[1].role, AccountRole.WRITABLE);
    assert.equal(accounts[2].address, vault.address);
  });

  it("uses the given address when the optional account is present", async () => {
    const signer = await generateKeyPairSigner();
    const note = await generateKeyPairSigner();
    const vault = await generateKeyPairSigner();
    const ix = await builder()
      .accounts({ signer: signer.address, note: note.address, vault: vault.address })
      .instruction();
    assert.equal(ix.accounts![1].address, note.address);
  });

  it("rejects an omitted optional account and says how to leave it out", async () => {
    const signer = await generateKeyPairSigner();
    const vault = await generateKeyPairSigner();
    await assert.rejects(
      builder().accounts({ signer: signer.address, vault: vault.address }).instruction(),
      /Account "note" was not provided.*pass null for "note"/,
    );
  });

  it("rejects null for an account that is not optional", () => {
    assert.throws(() => builder().accounts({ vault: null }), /"vault" is not optional/);
  });
});

describe("optional accounts: legacy builder", () => {
  const builder = () => new LegacyMethodsBuilder(idl, idl.instructions[0], {}, provider);

  it("fills a null optional account with the program id and keeps its writable flag", async () => {
    const signer = await Keypair.generate();
    const vault = await Keypair.generate();
    const ix = await builder()
      .accounts({ signer: signer.publicKey, note: null, vault: vault.publicKey })
      .instruction();
    assert.equal(ix.keys.length, 3);
    assert.equal(ix.keys[1].pubkey.toString(), PROGRAM_ID);
    assert.equal(ix.keys[1].isWritable, true);
    assert.equal(ix.keys[2].pubkey.toString(), vault.publicKey.toString());
  });

  it("uses the given address when the optional account is present", async () => {
    const signer = await Keypair.generate();
    const note = await Keypair.generate();
    const vault = await Keypair.generate();
    const ix = await builder()
      .accounts({
        signer: signer.publicKey,
        note: note.publicKey,
        vault: vault.publicKey,
      })
      .instruction();
    assert.equal(ix.keys[1].pubkey.toString(), note.publicKey.toString());
  });

  it("rejects an omitted optional account and says how to leave it out", async () => {
    const signer = await Keypair.generate();
    const vault = await Keypair.generate();
    await assert.rejects(
      builder().accounts({ signer: signer.publicKey, vault: vault.publicKey }).instruction(),
      /Account "note" was not provided.*pass null for "note"/,
    );
  });

  it("rejects null for an account that is not optional", () => {
    assert.throws(() => builder().accounts({ vault: null }), /"vault" is not optional/);
  });
});
