/**
 * @naclac/client — automatic PDA resolution: an account with a `pda` in the
 * IDL is derived when the caller omits it, against the program the PDA names
 * (an associated token account against the Associated Token Program, a
 * `seeds::program` account against that program), using the pinned addresses of
 * the instruction's other accounts.
 *
 * Run: pnpm test
 */

import { strict as assert } from "assert";
import { address, getProgramDerivedAddress } from "@solana/kit";
import { getBase58Encoder } from "@solana/codecs";
import { ATA_PROGRAM_ID, SYSTEM_PROGRAM_ID, TOKEN_2022_PROGRAM_ID, TOKEN_PROGRAM_ID } from "../src/constants";
import type { IdlInstruction, IdlSeed } from "../src/idl";
import { getAssociatedTokenAddress, resolvePdas } from "../src/utils/pda";

const PROGRAM = address("Vote111111111111111111111111111111111111111");
const OWNER = address("So11111111111111111111111111111111111111112");
const MINT = address("EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v");

function constSeed(base58: string): IdlSeed {
  return { kind: "const", value: [...getBase58Encoder().encode(base58)] };
}

function ataInstruction(): IdlInstruction {
  return {
    name: "take",
    discriminator: [],
    args: [],
    accounts: [
      { name: "taker" },
      { name: "mint_a" },
      { name: "token_program", address: TOKEN_PROGRAM_ID },
      {
        name: "taker_ata",
        pda: {
          seeds: [
            { kind: "account", path: "taker" },
            { kind: "account", path: "token_program" },
            { kind: "account", path: "mint_a" },
          ],
          program: constSeed(ATA_PROGRAM_ID),
        },
      },
    ],
  } as IdlInstruction;
}

describe("automatic PDA resolution", () => {
  it("derives an associated token account against the Associated Token Program", async () => {
    const resolved = await resolvePdas(PROGRAM, ataInstruction(), {}, { taker: OWNER, mint_a: MINT });
    const [expected] = await getAssociatedTokenAddress(MINT, OWNER, TOKEN_PROGRAM_ID);
    assert.equal(resolved.taker_ata, expected);
  });

  it("uses the token program the caller passes over the pinned one", async () => {
    const tokenProgram = address(TOKEN_2022_PROGRAM_ID);
    const resolved = await resolvePdas(PROGRAM, ataInstruction(), {}, {
      taker: OWNER,
      mint_a: MINT,
      token_program: tokenProgram,
    });
    const [expected] = await getAssociatedTokenAddress(MINT, OWNER, TOKEN_2022_PROGRAM_ID);
    assert.equal(resolved.taker_ata, expected);
  });

  it("keeps an account the caller passed explicitly", async () => {
    const explicit = address(SYSTEM_PROGRAM_ID);
    const resolved = await resolvePdas(PROGRAM, ataInstruction(), {}, {
      taker: OWNER,
      mint_a: MINT,
      taker_ata: explicit,
    });
    assert.equal(resolved.taker_ata, undefined);
  });

  it("derives a seeds::program account against that program, not the program itself", async () => {
    const foreign = TOKEN_PROGRAM_ID;
    const ix = {
      name: "make",
      discriminator: [],
      args: [],
      accounts: [
        {
          name: "foreign_pda",
          pda: { seeds: [{ kind: "const", value: [1, 2, 3] }], program: constSeed(foreign) },
        },
        { name: "own_pda", pda: { seeds: [{ kind: "const", value: [1, 2, 3] }] } },
      ],
    } as IdlInstruction;

    const resolved = await resolvePdas(PROGRAM, ix, {}, {});
    const seeds = [new Uint8Array([1, 2, 3])];
    const [underForeign] = await getProgramDerivedAddress({ programAddress: address(foreign), seeds });
    const [underOwn] = await getProgramDerivedAddress({ programAddress: PROGRAM, seeds });

    assert.equal(resolved.foreign_pda, underForeign);
    assert.equal(resolved.own_pda, underOwn);
    assert.notEqual(resolved.foreign_pda, resolved.own_pda);
  });

  it("refuses to derive against a program that is not a constant", async () => {
    const ix = {
      name: "make",
      discriminator: [],
      args: [],
      accounts: [
        {
          name: "odd_pda",
          pda: {
            seeds: [{ kind: "const", value: [1] }],
            program: { kind: "account", path: "some_program" },
          },
        },
      ],
    } as IdlInstruction;

    await assert.rejects(() => resolvePdas(PROGRAM, ix, {}, {}), /not a constant/);
  });
});
