import { createHash } from 'node:crypto';

export function verifyIntegrity(bytes, integrity) {
  const tokens = String(integrity).split(/\s+/).filter(Boolean);
  for (const algorithm of ['sha512', 'sha384', 'sha256', 'sha1']) {
    const supported = tokens.filter(token => token.startsWith(`${algorithm}-`));
    if (!supported.length) continue;
    const actual = createHash(algorithm).update(bytes).digest('base64');
    if (!supported.some(token => token === `${algorithm}-${actual}`)) throw new Error('Dependency integrity mismatch');
    return;
  }
  throw new Error('Missing or unsupported dependency integrity');
}

export const sha256 = bytes => createHash('sha256').update(bytes).digest('hex');
