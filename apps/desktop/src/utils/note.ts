/** 与 Rust `files::MAX_NOTE_BYTES` 保持一致。 */
export const MAX_NOTE_BYTES = 64 * 1024

/** 文本投递限制按 UTF-8 字节计算，而不是 JavaScript 字符数。 */
export function utf8ByteLength(value: string): number {
  return new TextEncoder().encode(value).length
}

export function noteIsWithinLimit(value: string): boolean {
  return utf8ByteLength(value) <= MAX_NOTE_BYTES
}
