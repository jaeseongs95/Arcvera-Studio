// Package only a committed candidate; never include untracked files or user data.
import fs from 'node:fs';
import path from 'node:path';
import { execFileSync } from 'node:child_process';
import { verifyIntegrity, sha256 } from './source-integrity.mjs';

const run = (file, args, cwd) => execFileSync(file, args, { cwd, encoding: 'utf8', windowsHide: true, maxBuffer: 32 * 1024 * 1024 });
const repo = process.cwd();
const revision = run('git', ['rev-parse', 'HEAD'], repo).trim();
if (!/^[a-f0-9]{40}$/.test(revision)) throw new Error('Exact source revision required');
const changes = run('git', ['diff', '--name-only', 'HEAD'], repo).trim().split(/\r?\n/).filter(Boolean);
// notices.mjs regenerates the dependency notices for the packaging platform.
if (changes.some(file => file !== 'THIRD_PARTY_NOTICES.md')) throw new Error('Commit tracked changes before packaging source');
const output = path.resolve(process.argv[2] ?? 'artifacts'); fs.mkdirSync(output, { recursive: true });
const stagingRoot = process.env.RUNNER_TEMP ?? path.join(repo, 'src-tauri', 'target');
fs.mkdirSync(stagingRoot, { recursive: true });
const temp = fs.mkdtempSync(path.join(stagingRoot, 'arcvera-source-'));
const source = path.join(temp, 'source'); fs.mkdirSync(source);
const tracked = path.join(temp, 'tracked.zip');
run('git', ['archive', '--format=zip', `--output=${tracked}`, revision], repo);
run('tar', ['-xf', tracked, '-C', source], repo);
fs.copyFileSync(path.join(repo, 'THIRD_PARTY_NOTICES.md'), path.join(source, 'THIRD_PARTY_NOTICES.md'));
const pkg = JSON.parse(fs.readFileSync(path.join(source, 'package.json')));
if (pkg.license !== 'AGPL-3.0-or-later') throw new Error('All releases from 2.0.0 must retain AGPL-3.0-or-later');
fs.writeFileSync(path.join(source, 'SOURCE-REVISION'), revision + '\n');
const config = run('cargo', ['vendor', '--locked', '--versioned-dirs', '--manifest-path', 'src-tauri/Cargo.toml', 'cargo-vendor'], source);
fs.mkdirSync(path.join(source, '.cargo'), { recursive: true });
const cargoConfig = path.join(source, '.cargo', 'config.toml');
if (fs.existsSync(cargoConfig)) throw new Error('Review existing Cargo source configuration before replacing it');
fs.writeFileSync(cargoConfig, config);
const lock = JSON.parse(fs.readFileSync(path.join(source, 'package-lock.json')));
const dependencies = []; fs.mkdirSync(path.join(source, 'npm-sources'));
for (const [location, dependency] of Object.entries(lock.packages)) {
  if (!location) continue;
  const url = new URL(dependency.resolved);
  if (url.protocol !== 'https:' || url.hostname !== 'registry.npmjs.org' || url.username || url.password) throw new Error(`Unsupported dependency source: ${location}`);
  const response = await fetch(url, { signal: AbortSignal.timeout(60000), redirect: 'error' });
  if (!response.ok) throw new Error(`Dependency download failed: ${location} HTTP ${response.status}`);
  const bytes = Buffer.from(await response.arrayBuffer()); verifyIntegrity(bytes, dependency.integrity);
  const digest = sha256(bytes); const file = `npm-sources/${digest}.tgz`;
  fs.writeFileSync(path.join(source, file), bytes);
  dependencies.push({ location, version: dependency.version, resolved: dependency.resolved, integrity: dependency.integrity, file, sha256: digest });
}
fs.writeFileSync(path.join(source, 'source-manifest.json'), JSON.stringify({ schemaVersion: 1, version: pkg.version, license: pkg.license, revision, dependencies }, null, 2) + '\n');
const archive = path.join(output, `arcvera-studio-${pkg.version}-source.zip`);
if (fs.existsSync(archive)) throw new Error('Refusing to overwrite an existing source archive');
run('tar', ['-a', '-cf', archive, '-C', source, '.'], repo);
const digest = sha256(fs.readFileSync(archive));
// Verify the published archive itself, not the directory used to create it.
const verify = path.join(temp, 'verify'); fs.mkdirSync(verify);
run('tar', ['-xf', archive, '-C', verify], repo);
run(process.execPath, ['scripts/restore-source-dependencies.mjs'], verify);
const npm = args => process.platform === 'win32'
  ? run(process.env.ComSpec ?? 'cmd.exe', ['/d', '/s', '/c', `npm.cmd ${args}`], verify)
  : run('npm', args.split(' '), verify);
npm('ci --offline --cache .source-npm-cache');
npm('run build');
run('cargo', ['metadata', '--offline', '--locked', '--format-version', '1', '--manifest-path', 'src-tauri/Cargo.toml'], verify);
fs.writeFileSync(path.join(output, 'source-build-info.json'), JSON.stringify({ revision, version: pkg.version, license: pkg.license, archive: path.basename(archive), sha256: digest, npmArchives: dependencies.length, offlineNpmInstall: true, offlineFrontendBuild: true, offlineCargoMetadata: true, nativeRebuild: 'NOT_RUN' }, null, 2) + '\n');
console.log(JSON.stringify({ archive, sha256: digest, npmArchives: dependencies.length, staging: source }));
