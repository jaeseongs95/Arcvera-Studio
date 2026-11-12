import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { spawnSync } from 'node:child_process';
import test from 'node:test';

const repository = fileURLToPath(new URL('../', import.meta.url));
const source = path.resolve(process.env.ARCVERA_RUNTIME_SOURCE || repository);
const read = file => fs.readFileSync(path.join(source, file), 'utf8');
const template = read('src-tauri/installer/installer.nsi');
const quote = text => text.replaceAll('$', () => '$$').replaceAll('"', '$\\"');

test('EXT-POLICY: installer never acquires or installs Microsoft runtimes', () => {
  for (const name of ['src-tauri/tauri.conf.json', 'src-tauri/tauri.bundle.conf.json']) {
    const config = JSON.parse(read(name));
    if (name.endsWith('tauri.conf.json')) assert.equal(config.bundle.windows.webviewInstallMode?.type, 'skip');
    else assert.equal(config.bundle.windows?.webviewInstallMode, undefined, 'bundle overlay must not override skip');
  }
  assert.doesNotMatch(template, /NSISdl::download|MicrosoftEdgeWebview2Setup|MicrosoftEdgeWebView2RuntimeInstaller|WEBVIEW2BOOTSTRAPPERPATH|WEBVIEW2INSTALLERPATH|ExecWait[^\r\n]*appguid=/i);
  const init = template.match(/^Function \.onInit\r?\n([\s\S]*?)^FunctionEnd/m)?.[1];
  assert.ok(init);
  assert.ok(init.indexOf('Call ArcveraRequireWebView2') >= 0);
  assert.ok(init.indexOf('Call ArcveraRequireWebView2') < init.indexOf('!insertmacro SetContext'));
  assert.doesNotMatch(template, /^Section WebView2/m, 'no later install/update branch may bypass the preflight');
});

test('EXT-BOUNDARY: exact NSIS prerequisite logic with explicit registry fixtures', () => {
  assert.equal(process.platform, 'win32');
  const compiler = process.env.ARCVERA_NSIS_COMPILER || [
    path.join(process.env.LOCALAPPDATA, 'tauri/NSIS/makensis.exe'),
    path.join(process.env.LOCALAPPDATA, 'tauri/NSIS/Bin/makensis.exe'),
  ].find(fs.existsSync);
  assert.ok(compiler, 'Verified NSIS compiler required');
  const matches = [...template.matchAll(/^Function ArcveraRequireWebView2\r?\n[\s\S]*?^FunctionEnd/gm)];
  assert.equal(matches.length, 1);
  const output = fs.mkdtempSync(path.join(repository, 'artifacts/external-runtime/nsis-'));
  const cases = [
    { id: 'absent', machine: '', user: '', minimum: '', pass: false },
    { id: 'zero', machine: '0.0.0.0', user: '', minimum: '', pass: false },
    { id: 'machine', machine: '130.0.1.0', user: '', minimum: '', pass: true },
    { id: 'user', machine: '', user: '130.0.1.0', minimum: '', pass: true },
    { id: 'zero-machine-user', machine: '0.0.0.0', user: '130.0.1.0', minimum: '', pass: true },
    { id: 'old', machine: '129.0.0.0', user: '', minimum: '130.0.1.0', pass: false },
    { id: 'equal', machine: '130.0.1.0', user: '', minimum: '130.0.1.0', pass: true },
    { id: 'newer', machine: '131.0.0.0', user: '', minimum: '130.0.1.0', pass: true },
    { id: 'missing-update', machine: '', user: '', minimum: '', pass: false, update: 1 },
    { id: 'missing-passive', machine: '', user: '', minimum: '', pass: false, passive: 1 },
  ];
  const results = [];
  for (const item of cases) {
    const marker = path.join(output, `${item.id}.complete`);
    const exe = path.join(output, `${item.id}.exe`);
    let registryReads = 0;
    // Explicit simulation boundary: replace registry reads only. No real registry writes,
    // full installer sections, network, user homes or shortcuts are present in this harness.
    const body = matches[0][0].replace(/ReadRegStr \$4 (HKLM|HKCU) [^\r\n]+/g, (_, hive) => {
      registryReads++;
      return `StrCpy $4 "${quote(hive === 'HKLM' ? item.machine : item.user)}"`;
    });
    assert.equal(registryReads, 3, 'all three production registry reads must be explicitly substituted');
    const script = `Unicode true
!include LogicLib.nsh
!include x64.nsh
!include WordFunc.nsh
!define WEBVIEW2APPGUID "{F3017226-FE2A-4295-8BDF-00C3A9A7E4C5}"
!define MINIMUMWEBVIEW2VERSION "${item.minimum}"
Name "Arcvera prerequisite fixture"
OutFile "${quote(exe)}"
RequestExecutionLevel user
SilentInstall silent
Var PassiveMode
Var UpdateMode
${body}
Function .onInit
StrCpy $PassiveMode ${item.passive || 0}
StrCpy $UpdateMode ${item.update || 0}
Call ArcveraRequireWebView2
FileOpen $9 "${quote(marker)}" w
FileWrite $9 "continued"
FileClose $9
SetErrorLevel 0
Quit
FunctionEnd
Section
SectionEnd
`;
    const input = path.join(output, `${item.id}.nsi`);
    // Match Tauri's generated UTF-8 BOM; NSIS otherwise uses the system code page.
    fs.writeFileSync(input, '\ufeff' + script);
    const compile = spawnSync(compiler, ['/V2', input], { encoding: 'utf8', windowsHide: true, timeout: 30000 });
    fs.writeFileSync(`${input}.compiler.txt`, `${compile.stdout || ''}\n${compile.stderr || ''}`);
    assert.equal(compile.status, 0, `${item.id}: ${compile.stdout} ${compile.stderr}`);
    const run = spawnSync(exe, ['/S'], { encoding: 'utf8', windowsHide: true, timeout: 15000 });
    assert.equal(run.error, undefined, item.id);
    assert.equal(run.status, item.pass ? 0 : 2, item.id);
    assert.equal(fs.existsSync(marker), item.pass, item.id);
    results.push({ ...item, exit: run.status, marker: fs.existsSync(marker) });
  }
  fs.writeFileSync(path.join(output, 'result.json'), JSON.stringify({ cases: results, boundary: 'Exact production NSIS function; registry inputs substituted. Actual installation, registry detection on a missing-runtime host and GUI NOT_RUN.' }, null, 2));
  for (const file of ['RUNTIME.md', 'RUNTIME.ko.md']) {
    const text = read(`src-tauri/installer/${file}`);
    assert.ok(text.includes('https://developer.microsoft.com/en-us/microsoft-edge/webview2/'));
    assert.ok(text.includes('https://learn.microsoft.com/en-us/cpp/windows/latest-supported-vc-redist'));
    assert.equal(JSON.parse(read('src-tauri/tauri.conf.json')).bundle.resources[`installer/${file}`], file);
    assert.equal(JSON.parse(read('src-tauri/tauri.bundle.conf.json')).bundle.resources[`installer/${file}`], file);
  }
});
