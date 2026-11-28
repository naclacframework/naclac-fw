import { strict as assert } from "assert";
import * as naclac from "@naclac-fw/client/legacy";
import { EventsBorshClient } from "../../artifacts/clients/typescript/src/generated/events_borsh/client-legacy";
import { EventsClient } from "../../artifacts/clients/typescript/src/generated/events/client-legacy";
import { EventsZcClient } from "../../artifacts/clients/typescript/src/generated/events_zc/client-legacy";

/**
 * `emit_cpi!` events never appear in a transaction's logs — they exist only
 * as inner instructions. These run the real programs in litesvm and read the
 * events back through the legacy client's `.rpc()` result.
 */
describe("emit_cpi! events (legacy client, litesvm)", () => {
  let payer: naclac.Keypair;

  before(async () => {
    payer = await naclac.loadNodeWallet();
  });

  const cases: { name: string; make: () => any }[] = [
    { name: "borsh", make: () => new EventsBorshClient("litesvm", payer) },
    { name: "pinocchio zero-copy", make: () => new EventsClient("litesvm", payer) },
    { name: "solana zero-copy", make: () => new EventsZcClient("litesvm", payer) },
  ];

  for (const { name, make } of cases) {
    it(`${name}: decodes the CPI-emitted events from .rpc()`, async () => {
      const client = make();
      const result = await client.eventCpiEmit({}, { program: client.programId }).rpc();

      assert.ok(result.innerInstructions, "litesvm must supply inner instructions");
      assert.equal(result.innerInstructions.length, 1);
      for (const call of result.innerInstructions) {
        assert.equal(call.programId, client.programId.toBase58());
        assert.equal(call.stackHeight, 2);
      }

      const counters = result.parseEvents("CounterIncremented");
      assert.equal(counters.length, 1);
      assert.equal(Number(counters[0].newCount), 7);

      // Logs alone can't see a CPI event (the sync typed helper is logs-only)...
      assert.equal(client.parseCounterIncrementedEvents(result.logs).length, 0);
      // ...but the async helper, given the inner instructions, can.
      const fromTransaction = await client.parseCounterIncrementedEventsFromTransaction(result);
      assert.equal(fromTransaction.length, 1);
      assert.equal(Number(fromTransaction[0].newCount), 7);
    });
  }
});
