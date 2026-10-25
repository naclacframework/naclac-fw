import * as naclac from "@naclac-fw/client/legacy";
import { CounterZcClient } from "../../artifacts/clients/typescript/src/generated/counter_zc/client-legacy";

describe("Naclac CounterZc Test Suite (Legacy)", () => {
  let client: CounterZcClient;
  let payer: naclac.Keypair;
  let counterPda: naclac.PublicKey;

  before(async () => {
    payer = await naclac.loadNodeWallet();
    client = new CounterZcClient("devnet", payer);
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

    // The event is a self-CPI (`emit_cpi!`), so it is in the transaction's
    // inner instructions, not its logs.
    const capturedEvents = client.parseCounterIncrementedEvents(
      txResult!.logs,
      txResult!.innerInstructions,
    );
    if (capturedEvents.length === 0) throw new Error("❌ Event was not captured!");

    console.log(`      🔔 Event Fired! New Count: ${capturedEvents[0].newCount}`);
  });

  it("3. Decrement and Verify Event", async () => {
    const stateBefore = await client.fetchCounter(counterPda);
    const startCount = Number(stateBefore.count);

    console.log(`      📉 Decrementing Counter (Current: ${startCount})...`);

    const txResult = await client
      .decrement({})
      .accounts({ authority: payer.publicKey, counterAccount: counterPda })
      .rpc()
      .catch(naclac.logError);

    const stateAfter = await client.fetchCounter(counterPda);
    console.log(`      ✅ New Count: ${stateAfter.count}`);

    if (Number(stateAfter.count) !== startCount - 1) {
      throw new Error("Count did not decrement correctly");
    }

    const capturedEvents = client.parseCounterDecrementedEvents(
      txResult!.logs,
      txResult!.innerInstructions,
    );
    if (capturedEvents.length === 0) throw new Error("❌ Event was not captured!");

    console.log(`      🔔 Event Fired! New Count: ${capturedEvents[0].newCount}`);
    console.log("      ✨ Test Suite Passed!");
  });
});
