import { strict as assert } from "assert";
import * as naclac from "@naclac-fw/client/kit";
import { EventsBorshClient } from "../clients/typescript/src/generated/events_borsh";
import { EventsClient } from "../clients/typescript/src/generated/events";
import { EventsZcClient } from "../clients/typescript/src/generated/events_zc";

/**
 * Same checks as `tests/kit/event_cpi.test.ts`, but against devnet: the inner
 * instructions come from a real node's `getTransaction` response. Needs
 * network access, a devnet-funded wallet, and the three events programs
 * deployed on devnet.
 */
describe("emit_cpi! events (kit client, devnet)", () => {
  let payer: naclac.KeyPairSigner;

  before(async () => {
    payer = await naclac.loadNodeWallet();
  });

  const cases: { name: string; make: () => any; alloc: boolean }[] = [
    { name: "borsh", make: () => new EventsBorshClient("devnet", payer), alloc: false },
    { name: "pinocchio zero-copy", make: () => new EventsClient("devnet", payer), alloc: true },
    { name: "solana zero-copy", make: () => new EventsZcClient("devnet", payer), alloc: true },
  ];

  for (const { name, make, alloc } of cases) {
    it(`${name}: decodes the CPI-emitted events from .rpc()`, async () => {
      const client = make();
      const result = await client.eventCpiEmit({}, { program: client.programId }).rpc();

      assert.ok(result.innerInstructions, "the node must supply inner instructions");
      assert.equal(result.innerInstructions.length, alloc ? 2 : 1);
      for (const call of result.innerInstructions) {
        assert.equal(call.programId, client.programId);
        assert.equal(call.stackHeight, 2);
      }

      const counters = result.parseEvents("CounterIncremented");
      assert.equal(counters.length, 1);
      assert.equal(Number(counters[0].newCount), 7);

      assert.equal(client.parseCounterIncrementedEvents(result.logs).length, 0);
      const fromTransaction = await client.parseCounterIncrementedEventsFromTransaction(result);
      assert.equal(fromTransaction.length, 1);
      assert.equal(Number(fromTransaction[0].newCount), 7);

      if (alloc) {
        const payloads = result.parseEvents("SizedPayload");
        assert.equal(payloads.length, 1);
        assert.deepEqual(Array.from(payloads[0].data as ArrayLike<number>), [1, 2, 3]);
      }
    });
  }
});
