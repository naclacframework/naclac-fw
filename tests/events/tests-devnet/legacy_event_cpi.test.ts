import { strict as assert } from "assert";
import * as naclac from "@naclac-fw/client/legacy";
import { EventsBorshClient } from "../clients/typescript/src/generated/events_borsh/client-legacy";
import { EventsClient } from "../clients/typescript/src/generated/events/client-legacy";
import { EventsZcClient } from "../clients/typescript/src/generated/events_zc/client-legacy";

/**
 * Same checks as `tests/legacy/event_cpi.test.ts`, but against devnet: the
 * inner instructions come from a real node's `getTransaction` response. Needs
 * network access, a devnet-funded wallet, and the three events programs
 * deployed on devnet.
 */
describe("emit_cpi! events (legacy client, devnet)", () => {
  let payer: naclac.Keypair;

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
        assert.equal(call.programId, client.programId.toBase58());
        assert.equal(call.stackHeight, 2);
      }

      const counters = result.parseEvents("CounterIncremented");
      assert.equal(counters.length, 1);
      assert.equal(Number(counters[0].newCount), 7);

      assert.equal(client.parseCounterIncrementedEvents(result.logs).length, 0);
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
});
