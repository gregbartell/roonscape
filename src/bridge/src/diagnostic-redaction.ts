import { isUtf8 } from "node:buffer";

/** Bound traversal before copying; never invoke getters or toJSON from input. */
export function redactDiagnosticData(value: unknown, limit: number): unknown {
  let remaining = limit;
  const visit = (item: unknown, depth: number): unknown => {
    if (depth > 64 || --remaining < 0) throw new Error();
    if (typeof item === "string") {
      remaining -= item.length * 3;
      if (remaining < 0) throw new Error();
      return redactText(item);
    }
    if (Buffer.isBuffer(item)) {
      remaining -= item.length * 3;
      if (remaining < 0) throw new Error();
      const bytes = isUtf8(item)
        ? Buffer.from(redactText(item.toString("utf8")))
        : item;
      return { encoding: "base64", data: bytes.toString("base64") };
    }
    if (typeof item !== "object" || item === null) {
      if (
        typeof item === "bigint" ||
        typeof item === "function" ||
        typeof item === "symbol"
      )
        throw new Error();
      return item;
    }
    const result: Record<string, unknown> | unknown[] = Array.isArray(item)
      ? []
      : (Object.create(null) as Record<string, unknown>);
    for (const key in item) {
      if (!Object.hasOwn(item, key)) continue;
      remaining -= key.length * 3;
      if (remaining < 0) throw new Error();
      const descriptor = Object.getOwnPropertyDescriptor(item, key)!;
      if (!("value" in descriptor)) throw new Error();
      const redacted = sensitiveField(key);
      Object.defineProperty(result, key, {
        value: redacted ? "[REDACTED]" : visit(descriptor.value, depth + 1),
        enumerable: true,
        configurable: true,
        writable: true,
      });
    }
    return result;
  };
  return visit(value, 0);
}

function sensitiveField(key: string): boolean {
  return /token|password|passwd|secret|credential|authorization|authentication|cookie|apikey|privatekey|accesskey|sessionkey|signature|^(auth|pwd)$/.test(
    key.toLowerCase().replace(/[^a-z0-9]/g, ""),
  );
}

function redactText(value: string): string {
  return value
    .replace(/\b(Bearer|Basic)\s+[A-Za-z0-9+/_=.~-]+/gi, "$1 [REDACTED]")
    .replace(/(https?:\/\/)[^\s/@]+:[^\s/@]+@/gi, "$1[REDACTED]@")
    .replace(
      /([?&])([^=&\s]+)=([^&#\s]*)/g,
      (match: string, separator: string, key: string) =>
        sensitiveField(decodeURIComponent(key))
          ? `${separator}${key}=[REDACTED]`
          : match,
    );
}
