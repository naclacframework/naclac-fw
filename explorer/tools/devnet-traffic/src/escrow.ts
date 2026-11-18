/**
 * Escrow suite: drives every instruction of the five escrow example programs
 * on devnet (make, take, cancel), covering all three serializations and all
 * three transaction versions, plus failing cases: a zero amount, identical
 * mints, a duplicate escrow id, a wrong mint on take, the maker taking their
 * own escrow, a cancel by someone else, and taking an escrow that is gone.
 * Each program gets fresh mints and a fresh taker.
 */

import * as naclac from "@naclac-fw/client/kit";
import path from "node:path";
import { type Ctx, run } from "./chain";

const GENERATED = path.resolve(
  __dirname,
  "../../../../examples/escrow/artifacts/clients/typescript/src/generated",
);

const MAKER_TOKEN_A_SUPPLY = 5_000_000_000n;
const TAKER_TOKEN_B_SUPPLY = 2_000_000_000n;
const TAKER_SOL = 10_000_000n;

export const ESCROW_VARIANTS = [
  { program: "escrow", className: "EscrowClient" },
  { program: "escrow_borsh", className: "EscrowBorshClient" },
  { program: "escrow_wincode", className: "EscrowWincodeClient" },
  { program: "escrow_wincode_pinocchio", className: "EscrowWincodePinocchioClient" },
  { program: "escrow_zc", className: "EscrowZcClient" },
] as const;

/** Runs the escrow suite against each program in `variants`. */
export async function runEscrow(
  ctx: Ctx,
  variants: readonly (typeof ESCROW_VARIANTS)[number][],
): Promise<void> {
  for (const variant of variants) {
    console.log(`\nescrow suite: ${variant.program}`);
    const module = require(path.join(GENERATED, variant.program, "client-kit"));
    const client = new module[variant.className](ctx.provider);
    const maker = ctx.payer.address;
    const tokenProgram = naclac.TOKEN_PROGRAM_ID;
    const systemProgram = naclac.SYSTEM_PROGRAM_ID;
    const provider = ctx.provider;

    const newMint = async () =>
      naclac.createMint(provider, await naclac.generateKeyPairSigner(), tokenProgram, maker, 9);
    const mintA = await newMint();
    const mintB = await newMint();
    const mintC = await newMint();
    await naclac.mintTo(provider, mintA, maker, MAKER_TOKEN_A_SUPPLY, tokenProgram);
    await naclac.mintTo(provider, mintB, maker, TAKER_TOKEN_B_SUPPLY, tokenProgram);
    const [makerTokenAccountA] = await naclac.getAssociatedTokenAddress(mintA, maker, tokenProgram);
    const [makerTokenAccountB] = await naclac.getAssociatedTokenAddress(mintB, maker, tokenProgram);

    const taker = await naclac.generateKeyPairSigner();
    await naclac.transferSol(provider, taker.address, TAKER_SOL);
    const takerTokenAccountA = await naclac.createAta(provider, mintA, taker.address, tokenProgram);
    await naclac.mintTo(provider, mintB, taker.address, TAKER_TOKEN_B_SUPPLY, tokenProgram);
    const [takerTokenAccountB] = await naclac.getAssociatedTokenAddress(
      mintB,
      taker.address,
      tokenProgram,
    );
    const stranger = await naclac.generateKeyPairSigner();

    const base = {
      suite: "escrow",
      program: variant.program,
      programId: client.programId as string,
      idlErrors: client.program.idl.errors,
    };

    const startId = BigInt(Date.now()) * 100n;
    const prepare = async (offset: bigint) => {
      const id = startId + offset;
      const [escrowState] = await client.getEscrowStatePda({ maker, id });
      const vaultTokenAccount = await naclac.createAta(provider, mintA, escrowState, tokenProgram);
      return { id, escrowState, vaultTokenAccount };
    };

    type Escrow = Awaited<ReturnType<typeof prepare>>;
    const make = (e: Escrow, amountA: bigint, amountB: bigint, mintBOverride = mintB) => () =>
      client.make(
        { id: e.id, amountA, amountB },
        {
          maker,
          mintA,
          mintB: mintBOverride,
          escrowState: e.escrowState,
          vaultTokenAccount: e.vaultTokenAccount,
          makerTokenAccountA,
          tokenProgram,
          systemProgram,
        },
      );
    const cancel = (e: Escrow, signerAddress: string = maker, extra: any[] = []) => () =>
      client
        .cancel(
          { id: e.id },
          {
            maker: signerAddress,
            mintA,
            escrowState: e.escrowState,
            vaultTokenAccount: e.vaultTokenAccount,
            makerTokenAccountA,
            tokenProgram,
            systemProgram,
          },
        )
        .signers(extra);
    const take =
      (
        e: Escrow,
        overrides: Record<string, string> = {},
        signers: any[] = [taker],
      ) =>
      () =>
        client
          .take(
            { id: e.id },
            {
              taker: taker.address,
              maker,
              mintA,
              mintB,
              escrowState: e.escrowState,
              vaultTokenAccount: e.vaultTokenAccount,
              takerTokenAccountA,
              takerTokenAccountB,
              makerTokenAccountB,
              tokenProgram,
              systemProgram,
              ...overrides,
            },
          )
          .signers(signers);

    const cancelled = await prepare(1n);
    await run(ctx, { ...base, label: "make", intent: "success", version: "v1", build: make(cancelled, 400_000_000n, 800_000_000n) });
    await run(ctx, { ...base, label: "cancel", intent: "success", version: "v0", build: cancel(cancelled) });

    const taken = await prepare(2n);
    await run(ctx, { ...base, label: "make", intent: "success", version: "v1", build: make(taken, 500_000_000n, 1_000_000_000n) });
    await run(ctx, { ...base, label: "take", intent: "success", version: "legacy", build: take(taken) });

    const zero = await prepare(3n);
    await run(ctx, {
      ...base,
      label: "make with a zero amount",
      intent: "failure",
      version: "v1",
      note: "expected: the program's ZeroAmount error",
      build: make(zero, 0n, 1_000_000_000n),
    });

    const sameMint = await prepare(4n);
    await run(ctx, {
      ...base,
      label: "make with identical mints",
      intent: "failure",
      version: "v1",
      note: "expected: the program's InvalidMint error",
      build: make(sameMint, 100_000_000n, 100_000_000n, mintA),
    });

    const live = await prepare(5n);
    await run(ctx, { ...base, label: "make", intent: "success", version: "v1", build: make(live, 300_000_000n, 600_000_000n) });
    await run(ctx, {
      ...base,
      label: "make with an id that is already in use",
      intent: "failure",
      version: "v1",
      note: "expected: the escrow account already exists",
      build: make(live, 300_000_000n, 600_000_000n),
    });
    await run(ctx, {
      ...base,
      label: "take with the wrong mint B",
      intent: "failure",
      version: "v1",
      note: "expected: a mint mismatch (the program's InvalidMint or a framework token-mint constraint)",
      build: take(live, { mintB: mintC }),
    });
    await run(ctx, {
      ...base,
      label: "take where the taker is the maker",
      intent: "failure",
      version: "v1",
      note: "expected: a duplicate mutable account constraint",
      build: take(
        live,
        {
          taker: maker,
          takerTokenAccountA: makerTokenAccountA,
          takerTokenAccountB: makerTokenAccountB,
        },
        [],
      ),
    });
    await run(ctx, {
      ...base,
      label: "cancel by someone who is not the maker",
      intent: "failure",
      version: "v1",
      note: "expected: the escrow PDA does not match the signer",
      build: cancel(live, stranger.address, [stranger]),
    });
    await run(ctx, { ...base, label: "cancel", intent: "success", version: "v1", build: cancel(live) });
    await run(ctx, {
      ...base,
      label: "take an escrow that was already cancelled",
      intent: "failure",
      version: "v1",
      note: "expected: the escrow account no longer exists",
      build: take(live),
    });
  }
}
