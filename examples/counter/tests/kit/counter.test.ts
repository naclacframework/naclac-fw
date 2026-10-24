import * as naclac from "@naclac-fw/client/kit";
import { CounterClient } from "../../artifacts/clients/typescript/src/generated/counter/client-kit";

describe("Naclac Counter Test Suite (Kit)", () => {
  let client: CounterClient;
  let payer: naclac.KeyPairSigner;
  let counterPda: naclac.Address;

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
        .accounts({ payer: payer.address, counterAccount: counterPda })
        .rpc()
        .catch(naclac.logError);
    }
  });

  it("2. Increment and Verify Event", async () => {
    const stateBefore = await client.fetchCounter(counterPda);
    const startCount = Number(stateBefore.data.count);

    console.log(`      📈 Incrementing Counter (Current: ${startCount})...`);

    const txResult = await client
      .increment({})
      .accounts({ authority: payer.address, counterAccount: counterPda })
      .rpc()
      .catch(naclac.logError);

    const stateAfter = await client.fetchCounter(counterPda);
    console.log(`      ✅ New Count: ${stateAfter.data.count}`);

    if (Number(stateAfter.data.count) !== startCount + 1) {
      throw new Error("Count did not increment correctly");
    }

    // The event is a self-CPI (`emit_cpi!`), so it is in the transaction's
    // inner instructions, not its logs.
    const capturedEvents = await client.parseCounterIncrementedEventsFromTransaction(txResult!);
    if (capturedEvents.length === 0) throw new Error("❌ Event was not captured!");

    console.log(`      🔔 Event Fired! New Count: ${capturedEvents[0].newCount}`);
  });

  it("3. Decrement and Verify Event", async () => {
    const stateBefore = await client.fetchCounter(counterPda);
    const startCount = Number(stateBefore.data.count);

    console.log(`      📉 Decrementing Counter (Current: ${startCount})...`);

    const txResult = await client
      .decrement({})
      .accounts({ authority: payer.address, counterAccount: counterPda })
      .rpc()
      .catch(naclac.logError);

    const stateAfter = await client.fetchCounter(counterPda);
    console.log(`      ✅ New Count: ${stateAfter.data.count}`);

    if (Number(stateAfter.data.count) !== startCount - 1) {
      throw new Error("Count did not decrement correctly");
    }

    const capturedEvents = await client.parseCounterDecrementedEventsFromTransaction(txResult!);
    if (capturedEvents.length === 0) throw new Error("❌ Event was not captured!");

    console.log(`      🔔 Event Fired! New Count: ${capturedEvents[0].newCount}`);
    console.log("      ✨ Test Suite Passed!");
  });
});
