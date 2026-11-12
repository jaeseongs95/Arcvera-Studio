import fs from 'node:fs';
import path from 'node:path';
import { execFileSync } from 'node:child_process';
import { verifyIntegrity, sha256 } from './source-integrity.mjs';

const root = process.cwd();
const manifest = JSON.parse(fs.readFileSync('source-manifest.json'));
const lock = JSON.parse(fs.readFileSync('package-lock.json'));
if (manifest.revision !== fs.readFileSync('SOURCE-REVISION', 'utf8').trim()) throw new Error('Source revision mismatch');
if (Object.keys(lock.packages).length - 1 !== manifest.dependencies.length) throw new Error('Incomplete source dependency manifest');
const locations = new Set();
for (const entry of manifest.dependencies) {
  if (locations.has(entry.location)) throw new Error('Duplicate source dependency'); locations.add(entry.location);
  const dep = lock.packages[entry.location];
  if (!dep || dep.version !== entry.version || dep.resolved !== entry.resolved || dep.integrity !== entry.integrity) throw new Error('Dependency does not match lockfile');
  if (!/^npm-sources\/[a-f0-9]{64}\.tgz$/.test(entry.file)) throw new Error('Invalid dependency archive path');
  const file = path.join(root, entry.file); const bytes = fs.readFileSync(file);
  if (sha256(bytes) !== entry.sha256) throw new Error('Source archive hash mismatch');
  verifyIntegrity(bytes, entry.integrity);
  const args = ['cache', 'add', file, '--offline', '--cache', '.source-npm-cache'];
  if (process.platform === 'win32') {
    // The validated relative path contains no spaces or shell metacharacters.
    // Passing quoted absolute paths through cmd can leave literal quotes in npm's argv.
    execFileSync(process.env.ComSpec ?? 'cmd.exe', ['/d', '/s', '/c', `npm.cmd cache add ${entry.file} --offline --cache .source-npm-cache`], { stdio: 'inherit', windowsHide: true });
  } else execFileSync('npm', args, { stdio: 'inherit' });
}
console.log(`Verified and cached ${locations.size} dependency sources for ${manifest.revision}`);
