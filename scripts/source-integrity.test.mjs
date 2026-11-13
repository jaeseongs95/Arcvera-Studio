import { test } from 'node:test';
import assert from 'node:assert/strict';
import { createHash } from 'node:crypto';
import { verifyIntegrity } from './source-integrity.mjs';

const input = Buffer.from('abc');
const sha256 = 'sha256-' + Buffer.from('ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad', 'hex').toString('base64');
const sha512 = 'sha512-' + createHash('sha512').update(input).digest('base64');
test('known SHA256 vector is accepted', () => assert.doesNotThrow(() => verifyIntegrity(input, sha256)));
test('lockfile SHA512 integrity is accepted', () => assert.doesNotThrow(() => verifyIntegrity(input, sha512)));
test('modified dependency bytes are rejected', () => assert.throws(() => verifyIntegrity(Buffer.from('abd'), sha512), /mismatch/));
test('missing and unsupported integrity are rejected', () => {
  for (const value of ['', undefined, 'md5-value']) assert.throws(() => verifyIntegrity(input, value), /unsupported/);
});
test('valid weaker integrity cannot bypass invalid strongest integrity', () => assert.throws(() => verifyIntegrity(input, `${sha256} sha512-AAAA`), /mismatch/));
test('multiple tokens of the strongest algorithm use a matching token', () => assert.doesNotThrow(() => verifyIntegrity(input, `sha512-AAAA ${sha512}`)));
