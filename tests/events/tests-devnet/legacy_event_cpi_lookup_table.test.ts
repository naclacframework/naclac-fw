import { strict as assert } from "assert";
import { TransactionMessage, VersionedTransaction } from "@solana/web3.js";
import * as naclac from "@naclac-fw/client/legacy";
import { EventsClient } from "../artifacts/clients/typescript/src/generated/events/client-legacy";
import { createLookupTable } from "./lookup-table";

/**
 * A v0 transaction that reaches the program's `event_authority` through an
 * address lookup table, so the inner instructions' account indices point into
 * the loaded-address section. Runs against devnet with the `events` program
 * deployed there; needs network access and a devnet-funded wallet.
 */
describe("emit_cpi! events through an address lookup table (legacy client, devnet)", () => {
  it("decodes the CPI-emitted event of a v0 transaction", async () => {
    const payer = await naclac.loadNodeWallet();
    const client = new EventsClient("devnet", payer);
    const connection = new naclac.Connection("https://api.devnet.solana.com", "confirmed");
    const eventAuthority = naclac.PublicKey.findProgramAddressSync(
      [Buffer.from("__event_authority")],
      client.programId,
    )[0];

    const table = await createLookupTable(connection, payer, [eventAuthority]);
    const tableAccount = (await connection.getAddressLookupTable(table)).value;
    assert.ok(tableAccount, "the lookup table must exist");

    const instruction = await client
      .eventCpiEmit({}, { program: client.programId })
      .instruction();
    const { blockhash, lastValidBlockHeight } = await connection.getLatestBlockhash();
    const message = new TransactionMessage({
      payerKey: payer.publicKey,
      recentBlockhash: blockhash,
      instructions: [instruction],
    }).compileToV0Message([tableAccount]);
    assert.equal(message.addressTableLookups.length, 1);
    assert.ok(
      !message.staticAccountKeys.some((key) => key.equals(eventAuthority)),
      "event_authority must be reached through the lookup table, not the static keys",
    );

    const transaction = new VersionedTransaction(message);
    transaction.sign([payer]);
    const signature = await connection.sendTransaction(transaction);
    await connection.confirmTransaction({ signature, blockhash, lastValidBlockHeight }, "confirmed");

    const result = await connection.getTransaction(signature, {
      commitment: "confirmed",
      maxSupportedTransactionVersion: 0,
    });
    const innerInstructions = naclac.innerInstructionsFromLegacyTransaction(result);
    assert.ok(innerInstructions, "inner instructions must resolve through the lookup table");
    assert.equal(innerInstructions.length, 2);
    for (const call of innerInstructions) {
      assert.equal(call.programId, client.programId.toBase58());
      assert.deepEqual(call.accounts, [eventAuthority.toBase58()]);
    }

    const counters = client.parseCounterIncrementedEvents(
      result?.meta?.logMessages ?? [],
      innerInstructions,
    );
    assert.equal(counters.length, 1);
    assert.equal(Number(counters[0].newCount), 7);
  });
});
