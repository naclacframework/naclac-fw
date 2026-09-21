import * as naclac from "@naclac-fw/client/legacy";
import { CounterClient } from "../../clients/typescript/src/generated/counter/client-legacy";

describe("Naclac Counter Test Suite (Legacy)", () => {
  let client: CounterClient;
  let payer: naclac.Keypair;
  let counterPda: naclac.PublicKey;

  before(async () => {
    payer = await naclac.loadNodeWallet();
    // Change this to "devnet" or an RPC URL to test against a live network!
    client = new CounterClient("litesvm", payer);
    [counterPda] = await client.getCounterAccountPda({});
  });

  it("1. Setup & Initialize Counter", async () => {
    console.log("      🔍 Checking if Counter exists...");
    try {
      await client.fetchCounter(counterPda);
      console.log("      ℹ️  Counter already exists — skipping initialization.");
    } catch (e) {
      console.log("      🚀 Initializing New Counter...");
      await client
        .initialize({})
        .accounts({ payer: payer.publicKey, counterAccount: counterPda })
        .rpc()
        .catch(naclac.logError);
    }
  });

  it("2. Increment and Verify Event", async () => {
    const stateBefore = await client.fetchCounter(counterPda);
    const startCount = Number(stateBefore.count);

    console.log(`      📈 Incrementing Counter (Current: ${startCount})...`);

    const txResult = await client
      .increment({})
      .accounts({ authority: payer.publicKey, counterAccount: counterPda })
      .rpc()
      .catch(naclac.logError);

    const stateAfter = await client.fetchCounter(counterPda);
    console.log(`      ✅ New Count: ${stateAfter.count}`);

    if (Number(stateAfter.count) !== startCount + 1) {
      throw new Error("Count did not increment correctly");
    }

    // Race-free: reads events straight out of the already-confirmed
    // transaction's own logs, instead of racing a live subscription.
    const capturedEvents = client.parseCounterIncrementedEvents(txResult!.logs);
    if (capturedEvents.length === 0) throw new Error("❌ Event was not captured!");

    console.log(`      🔔 Event Fired! New Count: ${capturedEvents[0].newCount}`);
    console.log("      ✨ Test Suite Passed!");
  });

  it("3. Event listener fires via emulated notification", async () => {
    // litesvm's blockhash doesn't advance on its own the way a real
    // cluster's does — sending the exact same instruction again under the
    // same blockhash produces an identical signature, which litesvm
    // correctly rejects as already-processed (real replay protection).
    client.program.provider.litesvm?.expireBlockhash();

    let receivedEvent: any = null;
    const listenerId = client.program.addEventListener("CounterIncremented", (event) => {
      receivedEvent = event;
    });

    await client
      .increment({})
      .accounts({ authority: payer.publicKey, counterAccount: counterPda })
      .rpc()
      .catch(naclac.logError);

    client.program.removeEventListener(listenerId);

    if (!receivedEvent) throw new Error("❌ addEventListener never fired for litesvm mode!");
    console.log(`      🔔 addEventListener fired! New Count: ${receivedEvent.newCount}`);
  });
});
