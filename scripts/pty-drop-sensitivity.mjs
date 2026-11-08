import { createHash } from 'node:crypto';
import { cpSync, mkdirSync, mkdtempSync, readFileSync, readdirSync, writeFileSync } from 'node:fs';
import { spawnSync } from 'node:child_process';
import path from 'node:path';
import { fileURLToPath, pathToFileURL } from 'node:url';

export const TEST = 'win::suspended_tests::dropping_uncommitted_launch_or_resume_token_reaps_the_real_target';
export const ARGV = ['cargo', 'test', '--locked', '--manifest-path', 'Cargo.toml', '--lib', TEST, '--', '--exact', '--nocapture', '--test-threads=1'];
const CRATE = 'src-tauri/vendor/portable-pty';
const SOURCE = `${CRATE}/src/win/suspended.rs`;
const TEST_FILE = `${CRATE}/src/win/suspended_tests.rs`;
const MODES = ['omit-termination', 'retain-process-handle', 'control'];
const TIMEOUT = 300_000;
const hash = bytes => `sha256:${createHash('sha256').update(bytes).digest('hex')}`;
const canonical = value => Array.isArray(value) ? `[${value.map(canonical).join(',')}]`
  : value && typeof value === 'object' ? `{${Object.keys(value).sort().map(key => `${JSON.stringify(key)}:${canonical(value[key])}`).join(',')}}`
    : JSON.stringify(value);
const seal = value => ({ ...value, digest: hash(canonical(value)) });
const json = (filename, value) => writeFileSync(filename, `${JSON.stringify(value, null, 2)}\n`);

// These faults exist only in disposable CI copies. Shipping sources are never edited.
export function mutate(source, mode) {
  if (!MODES.includes(mode)) throw new Error(`Unknown mutation: ${mode}`);
  if (mode === 'control') return source;
  const start = source.indexOf('impl Drop for ResumeToken {');
  if (start < 0 || source.indexOf('impl Drop for ResumeToken {', start + 1) >= 0) throw new Error('Expected one ResumeToken Drop');
  const prefix = source.slice(0, start);
  const drop = source.slice(start);
  const cleanup = /            if let Err\(error\) = Self::terminate_pending\(&process\) \{\r?\n                log::error!\("terminate\/reap uncommitted owned PTY: \{error\}"\);\r?\n            \}/g;
  if ([...drop.matchAll(cleanup)].length !== 1) throw new Error('Drop cleanup changed; review the mutation');
  const replacement = mode === 'omit-termination' ? '            drop(process);'
    : '$&\n            std::mem::forget(process);';
  return prefix + drop.replace(cleanup, replacement);
}

export function classify(mode, receipt) {
  if (!MODES.includes(mode)) throw new Error(`Unknown mutation: ${mode}`);
  if (!['PASS', 'FAIL'].includes(receipt.status)) return { accepted: false, reason: receipt.status };
  const log = `${receipt.stdout}\n${receipt.stderr}`;
  if (!/running 1 test\b/.test(log)) return { accepted: false, reason: 'Exact test did not run' };
  if (mode === 'control') return {
    accepted: receipt.exitCode === 0 && receipt.status === 'PASS' && log.includes(`test ${TEST} ... ok`)
      && /test result: ok\. 1 passed; 0 failed; 0 ignored;/.test(log),
    reason: 'Unmodified control must pass exactly one test (16 owned launches)',
  };
  const oracle = mode === 'omit-termination'
    ? /target did not terminate: whole_launch=false, iteration=0/
    : /cancelled token retained its process handle: whole_launch=false, iteration=0/;
  const witness = log.match(oracle)?.[0] ?? '';
  return {
    accepted: receipt.status === 'FAIL' && receipt.exitCode === 101 && witness !== ''
      && log.includes(TEST) && /test result: FAILED\. 0 passed; 1 failed; 0 ignored;/.test(log)
      && !/could not compile|error\[E\d+\]/.test(log),
    reason: 'Only the planned Rust assertion failure counts as a detected mutation', witness,
  };
}

function snapshot(crateDirectory) {
  const files = [SOURCE, TEST_FILE].sort().map(relative => {
    const bytes = readFileSync(path.join(crateDirectory, path.posix.relative(CRATE, relative)));
    const text = bytes.toString('utf8');
    return { path: relative, digest: hash(bytes), size: bytes.length,
      lineCount: text ? text.split('\n').length - Number(text.endsWith('\n')) : 0 };
  });
  return seal({ schemaVersion: '1.0.0', kind: 'engineering-snapshot', files });
}

function inventory(directory, relative = '') {
  return readdirSync(path.join(directory, relative), { withFileTypes: true }).sort((a, b) => a.name.localeCompare(b.name))
    .flatMap(entry => {
      if (entry.name === 'target' || entry.name === '.git') return [];
      const file = path.posix.join(relative, entry.name);
      if (entry.isDirectory()) return inventory(directory, file);
      if (!entry.isFile()) throw new Error(`Non-regular crate input: ${file}`);
      return [{ path: file, digest: hash(readFileSync(path.join(directory, file))) }];
    });
}

export function main() {
  if (process.platform !== 'win32' || process.env.GITHUB_ACTIONS !== 'true') {
    throw new Error('Native sensitivity execution is restricted to Windows GitHub Actions; local Rust is prohibited');
  }
  if (!process.env.RUNNER_TEMP) throw new Error('RUNNER_TEMP required');
  const repository = path.resolve(fileURLToPath(new URL('../', import.meta.url)));
  const evidence = path.join(repository, 'verification/drop-sensitivity-171');
  const output = path.join(evidence, 'runs');
  mkdirSync(output); // Refuse to overwrite any existing run evidence.
  const isolated = mkdtempSync(path.join(process.env.RUNNER_TEMP, 'drop-sensitivity-'));
  const original = path.join(repository, CRATE);
  const inputs = inventory(original);
  const candidate = snapshot(original);
  const planBytes = readFileSync(path.join(evidence, 'test-plan.json'));
  const plan = JSON.parse(planBytes);
  if (plan.cases.some(c => canonical(c.argv) !== canonical(ARGV))) throw new Error('Frozen argv differs from runner');
  const environmentNote = 'Windows GitHub Actions; sequential isolated crate copies; locked dependencies; distinct target directory per variant; CARGO_BUILD_JOBS=2; CARGO_INCREMENTAL=0; RUST_TEST_THREADS=1';
  const executions = [];
  for (const mode of MODES) {
    const copy = path.join(isolated, mode);
    cpSync(original, copy, { recursive: true, filter: source => !['target', '.git'].includes(path.basename(source)) });
    const sourceFile = path.join(copy, 'src/win/suspended.rs');
    if (mode !== 'control') writeFileSync(sourceFile, mutate(readFileSync(sourceFile, 'utf8'), mode));
    const copied = inventory(copy);
    const changed = copied.filter(file => inputs.find(input => input.path === file.path)?.digest !== file.digest).map(file => file.path);
    if (copied.length !== inputs.length || canonical(changed) !== canonical(mode === 'control' ? [] : ['src/win/suspended.rs'])) {
      throw new Error(`Unexpected crate input differences: ${mode}`);
    }
    const before = snapshot(copy);
    const startedAt = new Date().toISOString();
    const run = spawnSync(ARGV[0], ARGV.slice(1), {
      cwd: copy, encoding: 'utf8', timeout: TIMEOUT, maxBuffer: 2 * 1024 * 1024, windowsHide: true,
      env: { ...process.env, CARGO_BUILD_JOBS: '2', CARGO_INCREMENTAL: '0', RUST_TEST_THREADS: '1',
        CARGO_TARGET_DIR: path.join(isolated, `target-${mode}`) },
    });
    const after = snapshot(copy);
    const status = before.digest !== after.digest ? 'INPUT_CHANGED' : run.error?.code === 'ETIMEDOUT' ? 'TIMED_OUT'
      : run.error || run.signal || run.status === null ? 'SPAWN_ERROR' : run.status === 0 ? 'PASS' : 'FAIL';
    const receipt = seal({ schemaVersion: '1.0.0', kind: 'engineering-run-receipt', argv: ARGV,
      snapshot: before, afterDigest: after.digest, startedAt, finishedAt: new Date().toISOString(), status,
      exitCode: run.status, signal: run.signal, stdout: run.stdout ?? '', stderr: run.stderr ?? '',
      stdoutDigest: hash(run.stdout ?? ''), stderrDigest: hash(run.stderr ?? ''), timeoutMs: TIMEOUT,
      executable: ARGV[0], nodeVersion: process.version, platform: process.platform, environmentNote });
    const receiptFile = path.join(output, `${mode}.json`);
    json(receiptFile, receipt);
    writeFileSync(path.join(output, `${mode}.stdout.txt`), receipt.stdout);
    writeFileSync(path.join(output, `${mode}.stderr.txt`), receipt.stderr);
    const decision = classify(mode, receipt);
    executions.push({ mode, decision, receiptPath: `verification/drop-sensitivity-171/runs/${mode}.json`,
      receiptDigest: hash(readFileSync(receiptFile)), snapshot: before.digest,
      spawnError: run.error ? { code: run.error.code ?? null, message: run.error.message } : null,
      inputs: copied });
    console.log(JSON.stringify({ mode, status, exitCode: receipt.exitCode, ...decision }));
  }
  const green = executions.find(x => x.mode === 'control');
  const proofs = plan.cases.map((c, i) => {
    const red = executions[i];
    const demonstrated = red.decision.accepted && green.decision.accepted;
    const empty = { path: '', digest: hash('') };
    return { caseId: c.id, status: demonstrated ? 'DEMONSTRATED' : 'NOT_RUN',
      red: demonstrated ? { path: red.receiptPath, digest: red.receiptDigest } : empty,
      green: demonstrated ? { path: green.receiptPath, digest: green.receiptDigest } : empty,
      mutationPaths: demonstrated ? [SOURCE] : [], assertionWitness: demonstrated ? red.decision.witness : '',
      reason: demonstrated ? `Detected ${red.mode} at its planned assertion; same test bytes and argv; control follows red.`
        : `Planned assertion or control not established; inspect runs/${red.mode}.json and runs/control.json.` };
  });
  json(path.join(evidence, 'test-proof.json'), { schemaVersion: '1.0.0', kind: 'engineering-test-proof',
    planDigest: hash(canonical(plan)), candidate, proofs });
  const sourcePreserved = canonical(inputs) === canonical(inventory(original));
  const accepted = executions.every(x => x.decision.accepted) && sourcePreserved && green.snapshot === candidate.digest;
  json(path.join(evidence, 'result.json'), { commit: process.env.GITHUB_SHA, run: process.env.GITHUB_RUN_ID,
    accepted, sourcePreserved, candidate, planSha256: hash(planBytes), inputs, executions,
    limitations: ['Historical transient D0053 cause remains unconfirmed.', 'Not Desktop/daemon/GUI/TUI/installation validation.'] });
  if (!accepted) throw new Error('Sensitivity evidence incomplete; inspect preserved raw receipts');
}

if (process.argv[1] && import.meta.url === pathToFileURL(path.resolve(process.argv[1])).href) main();
