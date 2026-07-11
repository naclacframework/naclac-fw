import * as naclac from "@naclac-fw/client/legacy";
import { CounterBorshClient } from "../../clients/typescript/src/generated/counter_borsh/client-legacy";

describe("Naclac CounterBorsh Test Suite (Legacy)", () => {
  let client: CounterBorshClient;
  let payer: naclac.Keypair;
  let counterPda: naclac.PublicKey;

  before(async () => {
    payer = await naclac.loadNodeWallet();
    client = new CounterBorshClient("localnet", payer);
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
});
