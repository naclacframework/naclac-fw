const NATIVE_SOLANA_ERRORS: Record<string, string> = {
  "0x0": "Generic Instruction Error / Invalid Account Data",
  "0x1": "Insufficient Funds (Not enough SOL to pay for transaction or rent)",
  "0x2": "Invalid Account Data Format",
  "0x3": "Missing Required Signature",
  "0x4": "Account Data Too Small",
  "0x5": "Insufficient Funds For Rent",
  "0xbb8": "Account Already Initialized",
  "0xbbf": "Account Not Initialized",
};

const NACLAC_FRAMEWORK_ERROR_TYPES: Record<number, string> = {
  1: "Mutability mismatch",
  2: "Missing required signature",
  3: "Address mismatch",
  4: "Owner mismatch",
  5: "Account is not rent exempt",
  6: "PDA derivation failed (Seeds mismatch)",
  7: "Account is not executable",
  8: "Account is None (Expected initialized account)",
  9: "Program ID mismatch (Missing required program in accounts array)",
  10: "Account data too small (Missing discriminator or insufficient size)",
  11: "Account borrow failed (Already borrowed)",
  12: "Invalid instruction data",
  13: "Insufficient funds for transaction",
  14: "Account already initialized",
  15: "Account not initialized",
  16: "Not enough account keys provided",
  17: "Max seed length exceeded",
  18: "Unsupported sysvar",
  19: "Invalid reallocation",
  20: "Arithmetic overflow",
  21: "Unauthorized access (Authority mismatch)",
  22: "Account type mismatch (Invalid discriminator)",
  23: "Account deserialization failed",
  24: "Account serialization failed",
  25: "Duplicate mutable account detected in instruction",
  26: "Cannot close account to itself",
  27: "Too many PDA signers in one CPI call (exceeds MAX_CPI_SIGNERS)",
  28: "Too many seeds for one PDA signer in a CPI call (exceeds MAX_CPI_SEEDS_PER_SIGNER)",
};

/** A decoded program error from the IDL. */
export interface NaclacProgramError {
  code: number;
  name: string;
  msg?: string;
}

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

    // 3. Try to resolve from Naclac Framework Errors (Range 3000 - 3999)
    if (errorCode >= 3000 && errorCode < 4000) {
      const offset = errorCode - 3000;
      const errorType = offset % 100;
      const accountIndex = Math.floor(offset / 100);

      const typeName =
        NACLAC_FRAMEWORK_ERROR_TYPES[errorType] ?? "Unknown framework error";
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
