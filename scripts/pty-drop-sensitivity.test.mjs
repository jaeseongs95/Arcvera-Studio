import { test } from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { mutate, classify, TEST } from './pty-drop-sensitivity.mjs';

const source = readFileSync(new URL('../src-tauri/vendor/portable-pty/src/win/suspended.rs', import.meta.url), 'utf8');
test('mutations preserve every byte preceding Drop and control preserves all bytes', () => {
  assert.equal(mutate(source, 'control'), source);
  const prefix = source.slice(0, source.indexOf('impl Drop for ResumeToken {'));
  for (const mode of ['omit-termination', 'retain-process-handle']) assert.ok(mutate(source, mode).startsWith(prefix));
  assert.ok(!mutate(source, 'omit-termination').slice(prefix.length).includes('terminate_pending'));
  assert.equal(mutate(source, 'retain-process-handle'), source.replace(/(log::error!\("terminate\/reap uncommitted owned PTY: \{error\}"\);\r?\n            \})/, '$1\n            std::mem::forget(process);'));
  assert.throws(() => mutate(source.replace('impl Drop for ResumeToken {', 'impl Drop for ChangedToken {'), 'omit-termination'));
});

const red = marker => ({ status: 'FAIL', exitCode: 101, stdout: `running 1 test\ntest ${TEST} ... FAILED\nfailures:\n${TEST}\ntest result: FAILED. 0 passed; 1 failed; 0 ignored;`, stderr: marker });
const wait = 'target did not terminate: whole_launch=false, iteration=0';
const accounting = 'uncommitted target remained: active=1, whole_launch=false, iteration=0';
test('each mutant requires its own assertion witness', () => {
  assert.equal(classify('omit-termination', red(wait)).accepted, true);
  assert.equal(classify('retain-process-handle', red(accounting)).accepted, true);
  assert.equal(classify('retain-process-handle', red(wait)).accepted, false);
  assert.equal(classify('omit-termination', red(accounting)).accepted, false);
});
test('compiler, spawn and timeout failures cannot masquerade as a detected fault', () => {
  for (const status of ['SPAWN_ERROR', 'TIMED_OUT', 'INPUT_CHANGED']) assert.equal(classify('omit-termination', { ...red(wait), status }).accepted, false);
  assert.equal(classify('omit-termination', { ...red(wait), stdout: '', stderr: `could not compile: ${wait}` }).accepted, false);
  assert.equal(classify('omit-termination', { ...red(wait), stderr: `${wait}\nerror[E001]: compile error` }).accepted, false);
  assert.equal(classify('omit-termination', { ...red(wait), exitCode: 1 }).accepted, false);
});
test('control requires one executed passing test rather than exit zero alone', () => {
  const pass = { status: 'PASS', exitCode: 0, stdout: `running 1 test\ntest ${TEST} ... ok\ntest result: ok. 1 passed; 0 failed; 0 ignored;`, stderr: '' };
  assert.equal(classify('control', pass).accepted, true);
  assert.equal(classify('control', { ...pass, stdout: '' }).accepted, false);
  assert.equal(classify('control', { ...pass, stdout: pass.stdout.replace('1 passed', '0 passed') }).accepted, false);
});
