/**
 * Counter suite: drives every instruction of the five counter example programs
 * on devnet (initialize, increment, decrement), covering all three
 * serializations and all three transaction versions, plus failing cases:
 * initializing twice, a wrong authority, a wrong counter account, and a
 * decrement at zero.
 */

import * as naclac from "@naclac-fw/client/kit";
import path from "node:path";
import { type Ctx, run, skipped } from "./chain";

const GENERATED = path.resolve(
  __dirname,
  "../../../../examples/counter/artifacts/clients/typescript/src/generated",
);

const MAX_COUNT_TO_DRAIN = 12;

export const COUNTER_VARIANTS = [
  { program: "counter", className: "CounterClient" },
  { program: "counter_borsh", className: "CounterBorshClient" },
  { program: "counter_wincode", className: "CounterWincodeClient" },
  { program: "counter_wincode_pinocchio", className: "CounterWincodePinocchioClient" },
  { program: "counter_zc", className: "CounterZcClient" },
] as const;

/** Runs the counter suite against each program in `variants`. */
export async function runCounter(
  ctx: Ctx,
  variants: readonly (typeof COUNTER_VARIANTS)[number][],
): Promise<void> {
  for (const variant of variants) {
    console.log(`\ncounter suite: ${variant.program}`);
    const module = require(path.join(GENERATED, variant.program, "client-kit"));
    const client = new module[variant.className](ctx.provider);
    const [counter] = await client.getCounterAccountPda({});
    const payer = ctx.payer.address;
    const wrongSigner = await naclac.generateKeyPairSigner();
    const idlErrors = client.program.idl.errors;
    const base = {
      suite: "counter",
      program: variant.program,
      programId: client.programId as string,
      idlErrors,
    };

    const accounts = { authority: payer, counterAccount: counter };
    const increment = () => client.increment({}).accounts(accounts);
    const decrement = () => client.decrement({}).accounts(accounts);

    const existing = await client.fetchMaybeCounter(counter);
    if (existing.exists && existing.data.authority !== payer) {
      skipped(
        ctx,
        { ...base, label: "all steps", intent: "success" },
        `the counter's authority is ${existing.data.authority}, not the payer ${payer}`,
      );
      continue;
    }
    if (!existing.exists) {
      await run(ctx, {
        ...base,
        label: "initialize",
        intent: "success",
        version: "v1",
        build: () => client.initialize({}).accounts({ payer, counterAccount: counter }),
      });
    } else {
      skipped(ctx, { ...base, label: "initialize", intent: "success" }, "counter already exists");
    }

    await run(ctx, { ...base, label: "increment", intent: "success", version: "v1", build: increment });
    await run(ctx, { ...base, label: "increment", intent: "success", version: "v0", build: increment });
    await run(ctx, { ...base, label: "increment", intent: "success", version: "legacy", build: increment });
    const extraAccount = (await naclac.generateKeyPairSigner()).address;
    await run(ctx, {
      ...base,
      label: "increment with two extra accounts",
      intent: "success",
      version: "v1",
      note: "extra accounts are not named by the IDL",
      build: () => increment().remainingAccounts([wrongSigner.address, extraAccount]),
    });
    await run(ctx, {
      ...base,
      label: "increment with the authority repeated as an extra account",
      intent: "failure",
      version: "v1",
      note: "expected: the framework rejects a writable account passed twice",
      build: () => increment().remainingAccounts([ctx.payer.address]),
    });
    await run(ctx, { ...base, label: "decrement", intent: "success", version: "v0", build: decrement });

    await run(ctx, {
      ...base,
      label: "initialize a second time",
      intent: "failure",
      version: "v1",
      note: "expected: the account already exists",
      build: () => client.initialize({}).accounts({ payer, counterAccount: counter }),
    });
    await run(ctx, {
      ...base,
      label: "increment with the wrong authority",
      intent: "failure",
      version: "v1",
      note: "expected: the authority constraint fails",
      build: () =>
        client
          .increment({})
          .accounts({ authority: wrongSigner.address, counterAccount: counter })
          .signers([wrongSigner]),
    });
    await run(ctx, {
      ...base,
      label: "increment with the wrong counter account",
      intent: "failure",
      version: "v1",
      note: "expected: the counter account is not the program's PDA",
      build: () =>
        client.increment({}).accounts({ authority: payer, counterAccount: wrongSigner.address }),
    });

    let count = Number((await client.fetchCounter(counter)).data.count);
    const underflow = { ...base, label: "decrement at zero", intent: "failure" as const };
    if (count > MAX_COUNT_TO_DRAIN) {
      skipped(
        ctx,
        underflow,
        `count is ${count}, above ${MAX_COUNT_TO_DRAIN}; draining it would take too many transactions`,
      );
      continue;
    }
    for (let drained = 0; drained < MAX_COUNT_TO_DRAIN && count > 0; drained++) {
      await run(ctx, { ...base, label: "decrement", intent: "success", version: "v1", build: decrement });
      count = Number((await client.fetchCounter(counter)).data.count);
    }
    if (count > 0) {
      skipped(ctx, underflow, `count is still ${count} after draining`);
      continue;
    }
    await run(ctx, {
      ...underflow,
      version: "v1",
      note: "expected: the program's Underflow error",
      build: decrement,
    });
  }
}
