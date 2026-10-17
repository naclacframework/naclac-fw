import { decodeFrameworkError, NATIVE_SOLANA_ERRORS } from "./errors";

/**
 * Depth of the last "Program X invoke [N]" line in `text` — the stack depth
 * at which the actual failure most likely occurred. `1` means the top-level
 * submitted instruction; anything deeper means the failure originated inside
 * a CPI target, whose own account list `ixAccounts` (always the top-level
 * instruction's) cannot correctly describe — a valid-looking index there
 * names an unrelated account from the wrong instruction.
 */
function lastInvokeDepth(text: string): number | null {
  const matches = [...text.matchAll(/invoke \[(\d+)\]/g)];
  if (matches.length === 0) return null;
  return parseInt(matches[matches.length - 1][1], 10);
}

/**
 * Intercepts a raw @solana/kit RPC error and enriches it with human-readable
 * program error info from the IDL's errors array.
 *
 * If the error contains a Solana program custom error code of the form
 * "Custom program error: 0x1770" (where 0x1770 = 6000), it looks up the
 * matching entry in `idlErrors` and throws a new, enriched error.
 *
 * @param err       - The raw error thrown by the RPC.
 * @param idlErrors - The errors array from the IDL.
 */
export function translateRpcError(
  err: any,
  idlErrors: readonly { code: number; name: string; msg?: string }[],
  ixAccounts?: readonly { name: string }[],
): never {
  let currentErr = err;
  let combinedLogsAndMessages = "";
  const errorChainMessages: string[] = [];

  while (currentErr) {
    if (
      currentErr.message &&
      !errorChainMessages.includes(currentErr.message)
    ) {
      errorChainMessages.push(currentErr.message);
      combinedLogsAndMessages += currentErr.message + "\n";
    }

    if (currentErr.context?.logs && Array.isArray(currentErr.context.logs)) {
      combinedLogsAndMessages += currentErr.context.logs.join("\n") + "\n";
    }
    if (currentErr.logs && Array.isArray(currentErr.logs)) {
      combinedLogsAndMessages += currentErr.logs.join("\n") + "\n";
    }

    currentErr = currentErr.cause;
  }

  const hexMatch = combinedLogsAndMessages.match(
    /(?:(?:custom)? program error:|InstructionError(?:.*)?)\s*(0x[0-9a-fA-F]+)/i,
  );

  if (hexMatch) {
    const hexString = hexMatch[1].toLowerCase();
    const errorCode = parseInt(hexString, 16);

    const idlError = idlErrors.find((e) => e.code === errorCode);
    if (idlError) {
      const enriched = new Error(
        `[Naclac] Program error "${idlError.name}" (code ${idlError.code}): ${
          idlError.msg ?? "(no message)"
        }`,
      );
      enriched.cause = err; // Preserve the original error stack
      throw enriched;
    }

    const strippedHex = hexString.replace("0x", "");
    const normalizedKey = "0x" + parseInt(strippedHex, 16).toString(16);

    const nativeMsg = NATIVE_SOLANA_ERRORS[normalizedKey];
    if (nativeMsg) {
      const enriched = new Error(
        `[Naclac] Solana System Error: ${nativeMsg} (code ${normalizedKey})`,
      );
      enriched.cause = err;
      throw enriched;
    }

    // Naclac framework errors: 10000 + accountIndex * 100 + type, for account
    // indices 0..=255. User-defined error codes (6000+) always stay below 10000.
    const framework = decodeFrameworkError(errorCode);
    if (framework) {
      const { accountIndex } = framework;
      const typeName = framework.error?.message ?? "Unknown framework error";
      const depth = lastInvokeDepth(combinedLogsAndMessages);
      const isTopLevel = depth === null || depth === 1;
      const accountName = isTopLevel
        ? (ixAccounts?.[accountIndex]?.name ?? `Index ${accountIndex}`)
        : `Index ${accountIndex} (inside a nested CPI — the top-level instruction's own account list can't name it; check the logs for the real account)`;

      const enriched = new Error(
        `[Naclac] Framework Error: ${typeName} for account "${accountName}" (code ${errorCode})`,
      );
      enriched.cause = err;
      throw enriched;
    }
  }

  // 4. Try to match decimal "Custom" codes (e.g. from TEE or JSON status)
  const decimalMatch = combinedLogsAndMessages.match(/"Custom":\s*(\d+)/i);
  if (decimalMatch) {
    const errorCode = parseInt(decimalMatch[1], 10);
    const idlError = idlErrors.find((e) => e.code === errorCode);
    if (idlError) {
      const enriched = new Error(
        `[Naclac] Program error "${idlError.name}" (code ${idlError.code}): ${
          idlError.msg ?? "(no message)"
        }`,
      );
      enriched.cause = err;
      throw enriched;
    }
  }

  if (err instanceof Error && errorChainMessages.length > 1) {
    const deepMessage = errorChainMessages.join(" -> ");
    const enriched = new Error(deepMessage);
    enriched.cause = err.cause;
    if ((err as any).context) (enriched as any).context = (err as any).context;
    throw enriched;
  }

  throw err;
}
