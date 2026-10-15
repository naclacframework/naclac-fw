import { strict as assert } from "assert";
import eventsIdl from "../clients/typescript/src/generated/events/idl/events.json";
import eventsBorshIdl from "../clients/typescript/src/generated/events_borsh/idl/events_borsh.json";
import eventsZcIdl from "../clients/typescript/src/generated/events_zc/idl/events_zc.json";

/**
 * These three programs opt into the `idl-build` route (`naclac build` compiles
 * and runs the program to produce its IDL), so their generated IDLs pin what
 * that route reports: event-CPI accounts and tag, the `signer` keyword on a
 * non-`Signer` field, `address = crate::ID`, and doc comments.
 */
const programs: [string, any][] = [
  ["events", eventsIdl],
  ["events_borsh", eventsBorshIdl],
  ["events_zc", eventsZcIdl],
];

const instruction = (idl: any, name: string) => {
  const found = idl.instructions.find((ix: any) => ix.name === name);
  assert.ok(found, `instruction ${name}`);
  return found;
};

for (const [name, idl] of programs) {
  describe(`idl-build IDL (${name})`, () => {
    it("carries the event-CPI dispatch tag", () => {
      assert.deepEqual(idl.eventCpiTag, [34, 131, 189, 8, 189, 141, 224, 48]);
    });

    it("lists the injected event_authority and program accounts of an #[event_cpi] instruction", () => {
      const [authority, program] = instruction(idl, "event_cpi_emit").accounts;
      assert.equal(authority.name, "event_authority");
      assert.deepEqual(
        authority.pda.seeds.map((seed: any) => seed.value),
        [Array.from(Buffer.from("__event_authority"))],
      );
      assert.equal(program.name, "program");
      assert.equal(program.address, idl.address);
    });

    it("marks a #[account(signer)] AccountInfo field as a signer", () => {
      const [authority] = instruction(idl, "log_event_signed").accounts;
      assert.equal(authority.signer, true);
    });

    it("resolves address = crate::ID to the program's own id", () => {
      const program = instruction(idl, "emit_via_self_cpi_baseline").accounts.find(
        (a: any) => a.name === "program",
      );
      assert.equal(program.address, idl.address);
    });

    it("carries instruction and event doc comments", () => {
      assert.ok(instruction(idl, "emit_via_self_cpi_sized").docs.length > 0);
      const sized = idl.events.find((e: any) => e.name === "SizedPayload");
      assert.ok(sized.docs.length > 0);
    });
  });
}

describe("idl-build IDL (events, padded defined type)", () => {
  it("carries the doc comments of an event and of a defined type", () => {
    const nested = (eventsIdl as any).events.find((e: any) => e.name === "NestedPadded");
    assert.ok(nested.docs.length > 0);
    const padded = (eventsIdl as any).definedTypes.find((t: any) => t.name === "PaddedInner");
    assert.ok(padded.docs.length > 0);
  });
});
