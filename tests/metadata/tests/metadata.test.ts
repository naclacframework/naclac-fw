import * as naclac from "@naclac-fw/client";
import { MetadataClient } from "../clients/src/generated/metadata";

describe("Naclac Metadata Test Suite", () => {
  let client: MetadataClient;
  let payer: naclac.KeyPairSigner;
  let counterPda: naclac.Address;

  before(async () => {
    payer = await naclac.loadNodeWallet();
    client = new MetadataClient("localnet", payer);
    counterPda = await client.getCounterAccountPda({});
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
    const startCount = Number(stateBefore.count);

    console.log(`      📈 Incrementing Counter (Current: ${startCount})...`);

    // Start listening BEFORE the transaction
    const eventPromise = client.waitForCounterIncremented({ timeoutMs: 10000 });

    await client
      .increment({})
      .accounts({ authority: payer.address, counterAccount: counterPda })
      .rpc()
      .catch(naclac.logError);

    const stateAfter = await client.fetchCounter(counterPda);
    console.log(`      ✅ New Count: ${stateAfter.count}`);

    if (Number(stateAfter.count) !== startCount + 1) {
      throw new Error("Count did not increment correctly");
    }

    const capturedEvent = await eventPromise;
    if (!capturedEvent) throw new Error("❌ Event was not captured!"); 

    console.log(`      🔔 Event Fired! New Count: ${capturedEvent.newCount}`);
    console.log("      ✨ Test Suite Passed!");
  });
});
