/**
 * A UUIDv7 (RFC 9562): 48 bits of time in milliseconds, then random bits. A client that sends one
 * with a create request can retry the request safely.
 */
export function uuidv7(now: number = Date.now()): string {
  const bytes = crypto.getRandomValues(new Uint8Array(16));
  const time = BigInt(now);
  for (let index = 0; index < 6; index++) {
    bytes[index] = Number((time >> BigInt(8 * (5 - index))) & 0xffn);
  }
  bytes[6] = ((bytes[6] ?? 0) & 0x0f) | 0x70;
  bytes[8] = ((bytes[8] ?? 0) & 0x3f) | 0x80;
  const hex = Array.from(bytes, (byte) => byte.toString(16).padStart(2, "0")).join("");
  return `${hex.slice(0, 8)}-${hex.slice(8, 12)}-${hex.slice(12, 16)}-${hex.slice(16, 20)}-${hex.slice(20)}`;
}
