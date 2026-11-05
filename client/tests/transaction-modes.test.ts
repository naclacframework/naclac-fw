/**
 * Unit and differential tests for transaction version modes (V1, V0, Legacy)
 * across both the Kit (@solana/kit) and Legacy (@solana/web3.js v3) SDK implementations.
 */

import { strict as assert } from "assert";
import {
  Keypair,
  PublicKey,
  Transaction,
  VersionedTransaction,
  TransactionInstruction,
  MessageV1,
  type Blockhash,
} from "@solana/web3.js";
import { getBase64Codec } from "@solana/codecs";
import { generateKeyPairSigner, compileTransaction } from "@solana/kit";

import { LegacyMethodsBuilder, createProvider } from "../src/legacy";
import { pollForConfirmation } from "../src/legacy/transaction";
import { MethodsBuilder } from "../src/kit/methods";
import type { NaclacIdl } from "../src/idl";
import {
  type LegacyProvider,
  type TxVersion,
  normalizeTxVersion as normalizeLegacyTxVersion,
} from "../src/legacy/provider";
import {
  type NaclacProvider,
  normalizeTxVersion as normalizeKitTxVersion,
} from "../src/kit/provider";

const fixtureIdl: NaclacIdl = {
  address: "Fg6PaFpoGXkYsidMpWTK6W2BeZ7FEfcYkg476zPFsLnS",
  metadata: { name: "counter", version: "0.1.0", spec: "0.1.0" },
  instructions: [
    {
      name: "increment",
      discriminator: [11, 18, 104, 9, 104, 174, 59, 33],
      accounts: [
        { name: "authority", isMut: true, isSigner: true },
        { name: "counterAccount", isMut: true, isSigner: false },
      ],
      args: [{ name: "amount", type: "u64" }],
    },
  ],
  accounts: [],
  events: [],
  errors: [],
};

const DUMMY_BLOCKHASH = "11111111111111111111111111111111" as Blockhash;

async function randomPublicKey(): Promise<PublicKey> {
  return (await Keypair.generate()).publicKey;
}

describe("Transaction Modes & V1 Serialization", () => {
  describe("Legacy SDK (@solana/web3.js v3) — MessageV1 Wire Format", () => {
    it("serializes and deserializes a minimal MessageV1 roundtrip", async () => {
      const payer = await Keypair.generate();
      const counter = await Keypair.generate();

      const ix = new TransactionInstruction({
        programId: new PublicKey(fixtureIdl.address),
        keys: [
          { pubkey: payer.publicKey, isSigner: true, isWritable: true },
          { pubkey: counter.publicKey, isSigner: false, isWritable: true },
        ],
        data: new Uint8Array([1, 2, 3, 4]),
      });

      const message = MessageV1.compile({
        payerKey: payer.publicKey,
        recentBlockhash: DUMMY_BLOCKHASH,
        instructions: [ix],
      });

      const serialized = message.serialize();
      assert.equal(serialized[0], 0x81);

      const restored = MessageV1.deserialize(serialized);
      assert.equal(restored.version, 1);
      assert.equal(restored.recentBlockhash, DUMMY_BLOCKHASH);
      assert.equal(restored.staticAccountKeys.length, message.staticAccountKeys.length);
      assert.equal(
        restored.staticAccountKeys[0].toBase58(),
        payer.publicKey.toBase58(),
      );
      assert.equal(restored.compiledInstructions.length, 1);
      assert.deepEqual(
        restored.compiledInstructions[0].data,
        message.compiledInstructions[0].data,
      );
    });

    it("serializes and deserializes transaction config resource limits accurately", async () => {
      const payer = await Keypair.generate();
      const message = MessageV1.compile({
        payerKey: payer.publicKey,
        recentBlockhash: DUMMY_BLOCKHASH,
        instructions: [],
        transactionConfig: {
          computeUnitLimit: 450_000,
          loadedAccountsDataSizeLimit: 32 * 1024 * 1024,
          priorityFeeLamports: 12_500n,
          heapSize: 64 * 1024,
        },
      });

      const restored = MessageV1.deserialize(message.serialize());

      assert.equal(restored.transactionConfig?.computeUnitLimit, 450_000);
      assert.equal(
        restored.transactionConfig?.loadedAccountsDataSizeLimit,
        32 * 1024 * 1024,
      );
      assert.equal(restored.transactionConfig?.priorityFeeLamports, 12_500n);
      assert.equal(restored.transactionConfig?.heapSize, 64 * 1024);
    });

    it("serializes through VersionedTransaction with signature verification", async () => {
      const payer = await Keypair.generate();
      const message = MessageV1.compile({
        payerKey: payer.publicKey,
        recentBlockhash: DUMMY_BLOCKHASH,
        instructions: [],
      });

      const tx = new VersionedTransaction(message);
      await tx.sign([payer]);

      const rawWireBytes = tx.serialize();
      assert.ok(rawWireBytes.length > 0);

      const restoredTx = VersionedTransaction.deserialize(rawWireBytes);
      assert.equal(restoredTx.version, 1);
      assert.equal(restoredTx.signatures.length, 1);
      assert.notDeepEqual(
        restoredTx.signatures[0],
        new Uint8Array(64),
      );
    });
  });

  describe("Legacy SDK (@naclac-fw/client/legacy) — Builder Version Switching", () => {
    let blockhashFetches = 0;

    async function createMockLegacyProvider(defaultTxVersion?: TxVersion): Promise<{
      provider: LegacyProvider;
      payer: Keypair;
    }> {
      const payer = await Keypair.generate();
      blockhashFetches = 0;
      const provider: LegacyProvider = {
        connection: {
          getLatestBlockhash: async () => {
            blockhashFetches += 1;
            return {
              blockhash: DUMMY_BLOCKHASH,
              lastValidBlockHeight: 100n,
            };
          },
        } as any,
        payer,
        commitment: "confirmed",
        defaultTxVersion,
        getBalance: async () => 0n,
        getTokenBalance: async () => 0,
      };
      return { provider, payer };
    }

    it("defaults to V1 when defaultTxVersion is \"v1\" or undefined", async () => {
      const { provider, payer } = await createMockLegacyProvider();
      const counter = await randomPublicKey();

      const builder = new LegacyMethodsBuilder(
        fixtureIdl,
        fixtureIdl.instructions[0],
        { amount: 10n },
        provider,
      ).accounts({
        authority: payer.publicKey,
        counterAccount: counter,
      });

      const tx = await builder.transaction();
      assert.ok(tx instanceof VersionedTransaction);
      assert.equal(tx.version, 1);
    });

    it("switches to V0 when explicitly requested with .v0()", async () => {
      const { provider, payer } = await createMockLegacyProvider("v1");
      const counter = await randomPublicKey();

      const builder = new LegacyMethodsBuilder(
        fixtureIdl,
        fixtureIdl.instructions[0],
        { amount: 10n },
        provider,
      )
        .accounts({
          authority: payer.publicKey,
          counterAccount: counter,
        })
        .v0();

      const tx = await builder.transaction();
      assert.ok(tx instanceof VersionedTransaction);
      assert.equal(tx.version, 0);
    });

    it("switches to legacy Transaction when requested with .legacy()", async () => {
      const { provider, payer } = await createMockLegacyProvider("v1");
      const counter = await randomPublicKey();

      const builder = new LegacyMethodsBuilder(
        fixtureIdl,
        fixtureIdl.instructions[0],
        { amount: 10n },
        provider,
      )
        .accounts({
          authority: payer.publicKey,
          counterAccount: counter,
        })
        .legacy();

      const tx = await builder.transaction();
      assert.ok(tx instanceof Transaction);
      assert.equal(tx instanceof VersionedTransaction, false);
      assert.equal(tx.recentBlockhash, DUMMY_BLOCKHASH);
      assert.equal(tx.feePayer?.toBase58(), payer.publicKey.toBase58());
    });

    it("accepts version() with \"v0\", \"v1\" and \"legacy\"", async () => {
      const { provider, payer } = await createMockLegacyProvider();
      const counter = await randomPublicKey();

      const v0Tx = await new LegacyMethodsBuilder(
        fixtureIdl,
        fixtureIdl.instructions[0],
        { amount: 10n },
        provider,
      )
        .accounts({ authority: payer.publicKey, counterAccount: counter })
        .version("v0")
        .transaction();
      assert.ok(v0Tx instanceof VersionedTransaction && v0Tx.version === 0);

      const v1Tx = await new LegacyMethodsBuilder(
        fixtureIdl,
        fixtureIdl.instructions[0],
        { amount: 10n },
        provider,
      )
        .accounts({ authority: payer.publicKey, counterAccount: counter })
        .version("v1")
        .transaction();
      assert.ok(v1Tx instanceof VersionedTransaction && v1Tx.version === 1);

      const legacyTx = await new LegacyMethodsBuilder(
        fixtureIdl,
        fixtureIdl.instructions[0],
        { amount: 10n },
        provider,
      )
        .accounts({ authority: payer.publicKey, counterAccount: counter })
        .version("legacy")
        .transaction();
      assert.ok(legacyTx instanceof Transaction);
    });

    it("applies V1 compute unit, memory, and priority fee resource limits", async () => {
      const { provider, payer } = await createMockLegacyProvider("v1");
      const counter = await randomPublicKey();

      const tx = await new LegacyMethodsBuilder(
        fixtureIdl,
        fixtureIdl.instructions[0],
        { amount: 10n },
        provider,
      )
        .accounts({ authority: payer.publicKey, counterAccount: counter })
        .computeUnitLimit(300_000)
        .loadedAccountsDataSizeLimit(16 * 1024 * 1024)
        .priorityFee(5000)
        .transaction();

      assert.ok(tx instanceof VersionedTransaction);
      const msg = tx.message as MessageV1;
      assert.equal(msg.transactionConfig?.computeUnitLimit, 300_000);
      assert.equal(
        msg.transactionConfig?.loadedAccountsDataSizeLimit,
        16 * 1024 * 1024,
      );
      assert.equal(msg.transactionConfig?.priorityFeeLamports, 5000n);
    });

    for (const version of ["v1", "v0", "legacy"] as const) {
      it(`${version}: encoded() returns a transaction carrying the payer's signature`, async () => {
        const { provider, payer } = await createMockLegacyProvider(version);
        const counter = await randomPublicKey();

        const encoded = await new LegacyMethodsBuilder(
          fixtureIdl,
          fixtureIdl.instructions[0],
          { amount: 10n },
          provider,
        )
          .accounts({ authority: payer.publicKey, counterAccount: counter })
          .encoded();

        const wire = getBase64Codec().encode(encoded);
        if (version === "legacy") {
          const signature = Transaction.from(wire).signatures[0].signature;
          assert.ok(signature !== null && signature.some((byte) => byte !== 0));
        } else {
          const signature = VersionedTransaction.deserialize(wire).signatures[0];
          assert.ok(signature.some((byte) => byte !== 0));
        }
      });
    }

    it("fetches a single blockhash per signed send", async () => {
      const { provider, payer } = await createMockLegacyProvider("v1");
      const counter = await randomPublicKey();

      await new LegacyMethodsBuilder(
        fixtureIdl,
        fixtureIdl.instructions[0],
        { amount: 10n },
        provider,
      )
        .accounts({ authority: payer.publicKey, counterAccount: counter })
        .encoded();

      assert.equal(blockhashFetches, 1);
    });

    it("rpc() reports a failed log fetch through logsError and parseEvents throws", async () => {
      const { provider, payer } = await createMockLegacyProvider("v1");
      const connection = provider.connection as any;
      connection.sendRawTransaction = async () => "landedSignature";
      connection.getTransaction = async () => {
        throw new Error("rpc down");
      };
      provider.litesvm = {} as any;

      const result = await new LegacyMethodsBuilder(
        fixtureIdl,
        fixtureIdl.instructions[0],
        { amount: 10n },
        provider,
      )
        .accounts({ authority: payer.publicKey, counterAccount: await randomPublicKey() })
        .rpc();

      assert.equal(result.signature, "landedSignature");
      assert.deepEqual(result.logs, []);
      assert.match(result.logsError!.message, /rpc down/);
      assert.throws(
        () => result.parseEvents("Anything"),
        /Cannot parse events: fetching transaction landedSignature failed: rpc down/,
      );
    });

    it("createProvider(\"litesvm\") requires a payer", () => {
      assert.throws(() => createProvider("litesvm"), /requires a payer/);
    });

    it("pollForConfirmation rejects an unknown commitment", async () => {
      await assert.rejects(
        pollForConfirmation({} as any, "sig", 1n, "bogus" as any),
        /Unknown commitment/,
      );
    });
  });

  describe("Kit SDK (@naclac-fw/client/kit) — Builder Version Switching", () => {
    async function createMockKitProvider(defaultTxVersion?: TxVersion): Promise<{
      provider: NaclacProvider;
      signer: any;
    }> {
      const signer = await generateKeyPairSigner();
      const provider: NaclacProvider = {
        rpc: {
          getLatestBlockhash: () => ({
            send: async () => ({
              value: {
                blockhash: DUMMY_BLOCKHASH,
                lastValidBlockHeight: 100n,
              },
            }),
          }),
        } as any,
        rpcSubscriptions: {} as any,
        signer,
        commitment: "confirmed",
        defaultTxVersion,
        getBalance: async () => 0n,
        getTokenBalance: async () => 0,
      };
      return { provider, signer };
    }

    it("defaults to V1 transaction message", async () => {
      const { provider, signer } = await createMockKitProvider();
      const counter = (await generateKeyPairSigner()).address;

      const builder = new MethodsBuilder(
        fixtureIdl,
        fixtureIdl.instructions[0],
        { amount: 10n },
        provider,
      ).accounts({
        authority: signer.address,
        counterAccount: counter,
      });

      const txMsg = await builder.transaction();
      assert.equal(txMsg.version, 1);
      assert.equal(txMsg.config.computeUnitLimit, 1_400_000);
      assert.equal(
        txMsg.config.loadedAccountsDataSizeLimit,
        64 * 1024 * 1024,
      );

      const compiled = compileTransaction(txMsg);
      assert.equal(compiled.messageBytes[0], 0x81);
    });

    it("switches to V0 transaction message with .v0()", async () => {
      const { provider, signer } = await createMockKitProvider("v1");
      const counter = (await generateKeyPairSigner()).address;

      const builder = new MethodsBuilder(
        fixtureIdl,
        fixtureIdl.instructions[0],
        { amount: 10n },
        provider,
      )
        .accounts({
          authority: signer.address,
          counterAccount: counter,
        })
        .v0();

      const txMsg = await builder.transaction();
      assert.equal(txMsg.version, 0);

      const compiled = compileTransaction(txMsg);
      assert.equal(compiled.messageBytes[0], 0x80);
    });

    it("switches to legacy transaction message with .legacy()", async () => {
      const { provider, signer } = await createMockKitProvider("v1");
      const counter = (await generateKeyPairSigner()).address;

      const builder = new MethodsBuilder(
        fixtureIdl,
        fixtureIdl.instructions[0],
        { amount: 10n },
        provider,
      )
        .accounts({
          authority: signer.address,
          counterAccount: counter,
        })
        .legacy();

      const txMsg = await builder.transaction();
      assert.equal(txMsg.version, "legacy");

      const compiled = compileTransaction(txMsg);
      assert.ok(compiled.messageBytes[0] < 0x80);
    });

    it("applies V1 resource limits via chained builder methods", async () => {
      const { provider, signer } = await createMockKitProvider();
      const counter = (await generateKeyPairSigner()).address;

      const txMsg = await new MethodsBuilder(
        fixtureIdl,
        fixtureIdl.instructions[0],
        { amount: 10n },
        provider,
      )
        .accounts({ authority: signer.address, counterAccount: counter })
        .computeUnitLimit(250_000)
        .loadedAccountsDataSizeLimit(8 * 1024 * 1024)
        .priorityFee(8000n)
        .transaction();

      assert.equal(txMsg.version, 1);
      assert.equal(txMsg.config.computeUnitLimit, 250_000);
      assert.equal(
        txMsg.config.loadedAccountsDataSizeLimit,
        8 * 1024 * 1024,
      );
      assert.equal(txMsg.config.priorityFeeLamports, 8000n);
    });
  });

  describe("normalizeTxVersion", () => {
    const implementations = [
      ["kit", normalizeKitTxVersion],
      ["legacy", normalizeLegacyTxVersion],
    ] as const;

    for (const [name, normalize] of implementations) {
      it(`${name}: maps "v0", "v1", "legacy" and the default`, () => {
        assert.equal(normalize("v0"), 0);
        assert.equal(normalize("v1"), 1);
        assert.equal(normalize("legacy"), "legacy");
        assert.equal(normalize(undefined), 1);
      });

      it(`${name}: rejects any other value instead of falling back to V1`, () => {
        for (const bad of [0, 1, "0", "V0", "V1", "v2", "Legacy", "", null]) {
          assert.throws(
            () => normalize(bad as any),
            /Unknown transaction version/,
            `expected ${JSON.stringify(bad)} to be rejected`,
          );
        }
      });
    }

    it("legacy builder: .version() with an unknown value fails at transaction()", async () => {
      const payer = await Keypair.generate();
      const provider = {
        connection: {} as any,
        publicKey: payer.publicKey,
        payer,
        commitment: "confirmed",
        getBalance: async () => 0n,
        getTokenBalance: async () => 0,
      } as LegacyProvider;
      const builder = new LegacyMethodsBuilder(
        fixtureIdl,
        fixtureIdl.instructions[0],
        { amount: 10n },
        provider,
      )
        .accounts({ authority: payer.publicKey, counterAccount: await randomPublicKey() })
        .version(0 as any);
      await assert.rejects(builder.transaction(), /Unknown transaction version/);
    });
  });
});
