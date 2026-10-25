import { strict as assert } from "assert";
import {
  appendTransactionMessageInstruction,
  compressTransactionMessageUsingAddressLookupTables,
  createSolanaRpc,
  createSolanaRpcSubscriptions,
  createTransactionMessage,
  getSignatureFromTransaction,
  pipe,
  sendAndConfirmTransactionFactory,
  setTransactionMessageFeePayerSigner,
  setTransactionMessageLifetimeUsingBlockhash,
  signTransactionMessageWithSigners,
} from "@solana/kit";
import * as naclac from "@naclac-fw/client/kit";
import * as legacy from "@naclac-fw/client/legacy";
import { EventsClient } from "../artifacts/clients/typescript/src/generated/events";
import { createLookupTable } from "./lookup-table";

/**
 * A v0 transaction that reaches the program's `event_authority` through an
 * address lookup table, so the inner instructions' account indices point into
 * the loaded-address section. Runs against devnet with the `events` program
 * deployed there; needs network access and a devnet-funded wallet. The table
 * is created with web3.js because kit ships no lookup-table program helpers.
 */
describe("emit_cpi! events through an address lookup table (kit client, devnet)", () => {
  it("decodes the CPI-emitted event of a v0 transaction", async () => {
    const payer = await naclac.loadNodeWallet();
    const client = new EventsClient("devnet", payer);
    const [eventAuthority] = await naclac.getProgramDerivedAddress({
      programAddress: client.programId,
      seeds: [new TextEncoder().encode("__event_authority")],
    });

    const table = await createLookupTable(
      new legacy.Connection("https://api.devnet.solana.com", "confirmed"),
      await legacy.loadNodeWallet(),
      [new legacy.PublicKey(eventAuthority)],
    );

    const rpc = createSolanaRpc("https://api.devnet.solana.com");
    const rpcSubscriptions = createSolanaRpcSubscriptions("wss://api.devnet.solana.com");
    const instruction = await client
      .eventCpiEmit({}, { program: client.programId })
      .instruction();
    const { value: latestBlockhash } = await rpc.getLatestBlockhash().send();
    const message = pipe(
      createTransactionMessage({ version: 0 }),
      (m) => setTransactionMessageFeePayerSigner(payer, m),
      (m) => setTransactionMessageLifetimeUsingBlockhash(latestBlockhash, m),
      (m) => appendTransactionMessageInstruction(instruction as any, m),
      (m) =>
        compressTransactionMessageUsingAddressLookupTables(m as any, {
          [table.toBase58()]: [eventAuthority],
        } as any),
    );
    const signed = await signTransactionMessageWithSigners(message as any);
    await (sendAndConfirmTransactionFactory({ rpc, rpcSubscriptions } as any) as any)(signed, {
      commitment: "confirmed",
    });
    const signature = getSignatureFromTransaction(signed as any);

    const result = await rpc
      .getTransaction(signature, { commitment: "confirmed", maxSupportedTransactionVersion: 0 })
      .send();
    assert.ok(
      (result?.meta?.loadedAddresses?.readonly.length ?? 0) +
        (result?.meta?.loadedAddresses?.writable.length ?? 0) >
        0,
      "the node must report addresses loaded from the lookup table",
    );
    const innerInstructions = naclac.innerInstructionsFromKitTransaction(result as any);
    assert.ok(innerInstructions, "inner instructions must resolve through the lookup table");
    assert.equal(innerInstructions.length, 2);
    for (const call of innerInstructions) {
      assert.equal(call.programId, client.programId);
      assert.deepEqual(call.accounts, [eventAuthority]);
    }

    const counters = await client.parseCounterIncrementedEventsFromTransaction({
      logs: (result?.meta?.logMessages as string[] | undefined) ?? [],
      innerInstructions,
    });
    assert.equal(counters.length, 1);
    assert.equal(Number(counters[0].newCount), 7);
  });
});
