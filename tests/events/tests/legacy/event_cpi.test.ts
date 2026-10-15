import { strict as assert } from "assert";
import * as naclac from "@naclac-fw/client/legacy";
import { EventsBorshClient } from "../../clients/typescript/src/generated/events_borsh/client-legacy";
import { EventsClient } from "../../clients/typescript/src/generated/events/client-legacy";
import { EventsZcClient } from "../../clients/typescript/src/generated/events_zc/client-legacy";

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

  const cases: { name: string; make: () => any; alloc: boolean }[] = [
    { name: "borsh", make: () => new EventsBorshClient("litesvm", payer), alloc: false },
    { name: "pinocchio zero-copy", make: () => new EventsClient("litesvm", payer), alloc: true },
    { name: "solana zero-copy", make: () => new EventsZcClient("litesvm", payer), alloc: true },
  ];

  for (const { name, make, alloc } of cases) {
    it(`${name}: decodes the CPI-emitted events from .rpc()`, async () => {
      const client = make();
      const result = await client.eventCpiEmit({}, { program: client.programId }).rpc();

      assert.ok(result.innerInstructions, "litesvm must supply inner instructions");
      assert.equal(result.innerInstructions.length, alloc ? 2 : 1);
      for (const call of result.innerInstructions) {
        assert.equal(call.programId, client.programId.toBase58());
        assert.equal(call.stackHeight, 2);
      }

      const counters = result.parseEvents("CounterIncremented");
      assert.equal(counters.length, 1);
      assert.equal(Number(counters[0].newCount), 7);

      // The typed per-event helper: logs alone can't see a CPI event...
      assert.equal(client.parseCounterIncrementedEvents(result.logs).length, 0);
      // ...but with the inner instructions it can.
      assert.equal(
        client.parseCounterIncrementedEvents(result.logs, result.innerInstructions).length,
        1,
      );

      if (alloc) {
        const payloads = result.parseEvents("SizedPayload");
        assert.equal(payloads.length, 1);
        assert.deepEqual(Array.from(payloads[0].data as ArrayLike<number>), [1, 2, 3]);
      }
    });
  }

  it("pinocchio zero-copy: decodes an alloc event holding a padded defined type", async () => {
    const client = new EventsClient("litesvm", payer);
    const result = await client.eventCpiEmitNested({}, { program: client.programId }).rpc();

    const events = result.parseEvents("NestedPadded");
    assert.equal(events.length, 1);
    const event = events[0];
    assert.equal(Number(event.head), 5);
    assert.equal(Number(event.inner.kind), 3);
    assert.equal(BigInt(event.inner.amount), 0x1122334455667788n);
    assert.equal(event.many.length, 2);
    assert.equal(Number(event.many[0].kind), 9);
    assert.equal(BigInt(event.many[0].amount), 42n);
    assert.equal(Number(event.many[1].kind), 3);
    assert.equal(BigInt(event.many[1].amount), 0x1122334455667788n);
    assert.equal(event.note, "hi");
  });
});
