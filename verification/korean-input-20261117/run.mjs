import { spawn } from 'node:child_process';
import { mkdirSync, writeFileSync } from 'node:fs';
import { freemem } from 'node:os';
import { resolve } from 'node:path';

const variant = process.argv[2];
if (!['before', 'after'].includes(variant)) throw new Error('Expected before or after');
if (freemem() < 4 * 1024 ** 3) throw new Error('Insufficient free physical memory for the single-browser fixture');
const directory = `artifacts/korean-input-20261117/${variant}`;
mkdirSync(directory, { recursive: true });
const argv = ['node_modules/@playwright/test/cli.js', 'test', 'tests/terminal-flow.spec.ts', 'tests/conversation.spec.ts', '--grep', 'IME|Shift\\+Enter|Unicode|한글 조합', '--config', 'verification/korean-input-20261117/playwright.config.ts', '--workers=1', '--reporter=line,json'];
const startedAt = new Date().toISOString();
const child = spawn(process.execPath, argv, { windowsHide: true, env: { ...process.env, ARCVERA_IME_BASELINE: variant === 'before' ? '1' : '0', PLAYWRIGHT_JSON_OUTPUT_NAME: resolve(directory, 'report.json') }, stdio: ['ignore', 'pipe', 'pipe'] });
let stdout = '', stderr = '';
child.stdout.on('data', part => { stdout += part; process.stdout.write(part); });
child.stderr.on('data', part => { stderr += part; process.stderr.write(part); });
child.on('error', error => { console.error(error); process.exitCode = 1; });
child.on('exit', (code, signal) => {
  writeFileSync(`${directory}/stdout.log`, stdout);
  writeFileSync(`${directory}/stderr.log`, stderr);
  writeFileSync(`${directory}/execution.json`, JSON.stringify({ variant, argv: [process.execPath, ...argv], startedAt, finishedAt: new Date().toISOString(), code, signal, scope: 'Chromium/xterm with explicit native IPC fixture; not physical Windows IME or actual CLI' }, null, 2) + '\n');
  process.exitCode = code ?? 1;
});
