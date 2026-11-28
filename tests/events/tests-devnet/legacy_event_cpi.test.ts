import { strict as assert } from "assert";
import * as naclac from "@naclac-fw/client/legacy";
import { EventsBorshClient } from "../artifacts/clients/typescript/src/generated/events_borsh/client-legacy";
import { EventsClient } from "../artifacts/clients/typescript/src/generated/events/client-legacy";
import { EventsZcClient } from "../artifacts/clients/typescript/src/generated/events_zc/client-legacy";

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

  const cases: { name: string; make: () => any }[] = [
    { name: "borsh", make: () => new EventsBorshClient("devnet", payer) },
    { name: "pinocchio zero-copy", make: () => new EventsClient("devnet", payer) },
    { name: "solana zero-copy", make: () => new EventsZcClient("devnet", payer) },
  ];

  for (const { name, make } of cases) {
    it(`${name}: decodes the CPI-emitted events from .rpc()`, async () => {
      const client = make();
      const result = await client.eventCpiEmit({}, { program: client.programId }).rpc();

      assert.ok(result.innerInstructions, "the node must supply inner instructions");
      assert.equal(result.innerInstructions.length, 1);
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
    });
  }
});
