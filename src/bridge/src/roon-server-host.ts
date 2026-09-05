import { isIPv4 } from "node:net";

declare const roonServerHostBrand: unique symbol;

export type RoonServerHost = string & {
  readonly [roonServerHostBrand]: true;
};

export function parseRoonServerHost(value: string): RoonServerHost | null {
  if (isIPv4(value)) {
    return value as RoonServerHost;
  }
  const hostname = value.endsWith(".") ? value.slice(0, -1) : value;
  if (
    hostname.length === 0 ||
    hostname.length > 253 ||
    /^[\d.]+$/.test(hostname)
  ) {
    return null;
  }

  const labels = hostname.split(".");
  return labels.every(
    (label) =>
      label.length > 0 &&
      label.length <= 63 &&
      /^[a-z\d](?:[a-z\d-]*[a-z\d])?$/i.test(label),
  )
    ? (value as RoonServerHost)
    : null;
}
