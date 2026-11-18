/**
 * Reads a transaction's log messages for their structure: the program
 * invocations in execution order, the `Program data:` lines (the `emit!`
 * events) with the program that wrote each, and the first failure. Other log
 * lines are ignored.
 */

import { ParseError } from "./errors.ts";

export interface LogDataEntry {
  invocation: number;
  programId: string;
  chunks: string[];
}

export interface LogFailure {
  programId: string;
  reason: string;
}

export interface WalkedLogs {
  invocations: { programId: string; depth: number }[];
  data: LogDataEntry[];
  failure: LogFailure | null;
}

const INVOKE = /^Program (\S+) invoke \[(\d+)\]$/;
const SUCCESS = /^Program (\S+) success$/;
const FAILED = /^Program (\S+) failed: (.*)$/;
const DATA_PREFIX = "Program data: ";

function inconsistent(detail: string): never {
  throw new ParseError(`the program logs are inconsistent: ${detail}.`, "meta.logMessages");
}

/**
 * Walks `logs`. The k-th invocation is the k-th instruction executed, in the
 * same order as the flat instruction list. Throws a ParseError when invoke,
 * success and failed lines do not nest correctly, or data appears outside any
 * program.
 */
export function walkLogs(logs: string[]): WalkedLogs {
  const stack: string[] = [];
  const invocations: { programId: string; depth: number }[] = [];
  const data: LogDataEntry[] = [];
  let failure: LogFailure | null = null;

  for (const line of logs) {
    const invoke = INVOKE.exec(line);
    if (invoke) {
      const programId = invoke[1] as string;
      const depth = Number(invoke[2]);
      if (depth !== stack.length + 1) inconsistent(`invoke depth ${depth} at stack size ${stack.length}`);
      stack.push(programId);
      invocations.push({ programId, depth });
      continue;
    }
    const finished = SUCCESS.exec(line) ?? FAILED.exec(line);
    if (finished) {
      const programId = finished[1] as string;
      if (stack.pop() !== programId) inconsistent(`${programId} finished out of order`);
      if (failure === null && FAILED.test(line)) {
        failure = { programId, reason: (FAILED.exec(line)?.[2] as string) ?? "" };
      }
      continue;
    }
    if (line.startsWith(DATA_PREFIX)) {
      const programId = stack[stack.length - 1];
      if (programId === undefined) inconsistent("program data outside any program");
      data.push({
        invocation: invocations.length - 1,
        programId: programId as string,
        chunks: line.slice(DATA_PREFIX.length).split(" "),
      });
    }
  }
  return { invocations, data, failure };
}
