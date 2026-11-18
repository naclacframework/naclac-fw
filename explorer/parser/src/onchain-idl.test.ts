/**
 * End-to-end test of the whole path: the IDLs are read from the metadata
 * accounts recorded from devnet (not from local files), validated, and used to
 * parse every recorded counter transaction. The parse must equal the one made
 * with the local IDLs. Run with `node --test src/onchain-idl.test.ts`.
 */

import assert from "node:assert/strict";
import { test } from "node:test";
import { resolveIdl } from "../../idl/src/resolve.ts";
import { recordedMetadata } from "../../idl/src/test-support.ts";
import { parseNaclacIdl } from "./idl.ts";
import type { NaclacIdl } from "./idl.ts";
import { parseRawTransaction } from "./parse.ts";
import { counterIdls, devnetRaw, index } from "./test-support.ts";

const plain = (value: unknown): unknown => JSON.parse(JSON.stringify(value));

test("parsing with IDLs read from the chain equals parsing with the local IDLs, for all 65 transactions", async () => {
  const onchain = new Map<string, NaclacIdl>();
  for (const item of recordedMetadata()) {
    const result = await resolveIdl(item.programId, item.account);
    assert.equal(result.kind, "naclac", item.name);
    if (result.kind === "naclac") {
      const idl = parseNaclacIdl(result.document);
      onchain.set(idl.address, idl);
    }
  }
  assert.equal(onchain.size, 5);

  const local = counterIdls();
  let decoded = 0;
  for (const signature of Object.keys(index)) {
    const raw = devnetRaw(signature);
    const fromChain = parseRawTransaction(raw, { idls: onchain });
    assert.deepEqual(plain(fromChain), plain(parseRawTransaction(raw, { idls: local })), signature);
    assert.equal(fromChain.parsed.instructions[0]?.decodeStatus, "decoded", signature);
    decoded++;
  }
  assert.equal(decoded, 65);
});
