import assert from 'node:assert/strict';
import crypto from 'node:crypto';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { spawnSync } from 'node:child_process';
import test from 'node:test';
import { fileURLToPath } from 'node:url';

const repository = fileURLToPath(new URL('../', import.meta.url));
const hash = bytes => crypto.createHash('sha256').update(bytes).digest('hex');
const nsisString = value => value.replaceAll('$', () => '$$').replaceAll('"', '$\\"');

test('SC-ALL: actual NSIS shortcut functions preserve collisions and migration modes', () => {
  assert.equal(process.platform, 'win32', 'Windows CI is required');
  assert.equal(process.env.GITHUB_ACTIONS, 'true', 'Native fixture execution is limited to disposable CI');
  const source = path.resolve(process.env.ARCVERA_INSTALLER_SOURCE || repository);
  const label = process.env.ARCVERA_SHORTCUT_EVIDENCE || 'current';
  assert.match(label, /^[a-z0-9-]+$/);
  const evidence = path.join(repository, 'artifacts', 'installer-shortcuts', label);
  fs.mkdirSync(evidence, { recursive: true });
  const scratch = fs.mkdtempSync(path.join(process.env.RUNNER_TEMP || os.tmpdir(), 'arcvera-shortcut-'));
  const nsisRoot = path.join(process.env.LOCALAPPDATA, 'tauri', 'NSIS');
  const compiler = [path.join(nsisRoot, 'makensis.exe'), path.join(nsisRoot, 'Bin', 'makensis.exe')].find(fs.existsSync);
  assert.ok(compiler, 'Use the verified NSIS toolchain from the package build');
  const utils = path.join(repository, 'src-tauri', 'target', 'release', 'nsis', 'x64', 'utils.nsh');
  fs.copyFileSync(utils, path.join(evidence, 'utils.nsh'));
  // Tauri's Windows CLI embeds CRLF source and write_utf8_with_bom adds EF BB BF.
  // Official LF source SHA256: 78272bd40b8a783c0ab94e4bcb400fd947ba306a7575f2bbbbccb7478f40447f.
  // nsis/mod.rs writes these include_bytes unchanged after the BOM.
  assert.equal(hash(fs.readFileSync(utils)), '72cae7532ea22bd39b5433dfd8ac47975844defa916fc113880479dedfee54f6', 'Pinned Tauri 2.12.4 Windows generated utils (BOM + CRLF)');
  const template = fs.readFileSync(path.join(source, 'src-tauri/installer/installer.nsi'), 'utf8');
  const hooks = fs.readFileSync(path.join(source, 'src-tauri/installer/hooks.nsh'), 'utf8');
  const redirect = text => text.replaceAll('$SMPROGRAMS', '$FixtureStartMenu').replaceAll('$DESKTOP', '$FixtureDesktop');
  const functions = ['CreateOrUpdateStartMenuShortcut', 'CreateOrUpdateDesktopShortcut'].map(name => {
    const matches = [...template.matchAll(new RegExp(`^Function ${name}\\r?\\n[\\s\\S]*?^FunctionEnd`, 'gm'))];
    assert.equal(matches.length, 1, `Extract exactly one production function ${name}`);
    return redirect(matches[0][0]);
  }).join('\n');
  fs.writeFileSync(path.join(scratch, 'hooks.nsh'), redirect(hooks));
  const states = ['absent', 'current', 'recorded-old', 'foreign', 'corrupt', 'legacy-only', 'foreign-legacy', 'current-legacy'];
  const modes = ['finish', 'passive', 'update', 'no-shortcuts', 'wix'];
  const scenarios = [];
  for (const folder of ['', 'Tools']) for (const mode of modes) for (const state of states) {
    const id = `${folder ? 'folder' : 'root'}-${mode}-${state}`;
    const root = path.join(scratch, id);
    scenarios.push({ id, root, folder, mode, state, install: path.join(root, 'install'),
      desktop: path.join(root, 'desktop'), menu: path.join(root, 'menu'),
      links: [path.join(root, 'desktop', 'Arcvera-Studio.lnk'), path.join(root, 'menu', folder, 'Arcvera-Studio.lnk'),
        ...(folder ? [path.join(root, 'menu', 'Arcvera-Studio.lnk')] : [])],
      legacy: [path.join(root, 'desktop', 'OpenDIY Desktop.lnk'), path.join(root, 'menu', 'OpenDIY Desktop.lnk')] });
  }
  const manifest = path.join(scratch, 'scenarios.json');
  fs.writeFileSync(manifest, JSON.stringify(scenarios));
  const io = path.join(scratch, 'links.ps1');
  fs.writeFileSync(io, `param([string]$Manifest, [string]$Action)
$ErrorActionPreference = 'Stop'
$cases = Get-Content -LiteralPath $Manifest -Raw | ConvertFrom-Json
$shell = New-Object -ComObject WScript.Shell
$results = @()
foreach ($case in $cases) {
  if ($Action -eq 'create') {
    foreach ($dir in @($case.install, $case.desktop, $case.menu, (Join-Path $case.menu 'Tools'))) {
      New-Item -ItemType Directory -Path $dir -Force | Out-Null
    }
    foreach ($name in @('opendiy-desktop.exe', 'arcvera-studio.exe')) {
      [IO.File]::WriteAllText((Join-Path $case.install $name), 'inert fixture; never executed')
    }
    foreach ($link in $case.links) {
      if ($case.state -eq 'corrupt') { [IO.File]::WriteAllText($link, 'unreadable shortcut fixture'); continue }
      if ($case.state -in @('absent', 'legacy-only')) { continue }
      $shortcut = $shell.CreateShortcut($link)
      $target = 'arcvera-studio.exe'
      if ($case.state -eq 'recorded-old') { $target = 'opendiy-desktop.exe' }
      $shortcut.TargetPath = Join-Path $case.install $target
      if ($case.state -in @('foreign', 'foreign-legacy')) { $shortcut.TargetPath = Join-Path $case.root 'another-install/arcvera-studio.exe' }
      $shortcut.Arguments = '--keep-user-arguments'
      $shortcut.Description = 'User shortcut metadata'
      $shortcut.Save()
    }
    if ($case.state -in @('legacy-only', 'foreign-legacy', 'current-legacy')) {
      foreach ($link in $case.legacy) {
        $shortcut = $shell.CreateShortcut($link)
        $shortcut.TargetPath = Join-Path $case.install 'opendiy-desktop.exe'
        $shortcut.Arguments = '--keep-legacy-arguments'
        $shortcut.Save()
      }
    }
  } else {
    $links = @()
    foreach ($link in @($case.links) + @($case.legacy)) {
      $row = @{ path = $link; exists = (Test-Path -LiteralPath $link) }
      if ($row.exists -and $case.state -ne 'corrupt') {
        $shortcut = $shell.CreateShortcut($link)
        $row.target = $shortcut.TargetPath
        $row.arguments = $shortcut.Arguments
      }
      $links += $row
    }
    $results += @{ id = $case.id; links = $links }
  }
}
if ($Action -eq 'read') { ConvertTo-Json -Depth 6 -InputObject @($results) }
`);
  let sequence = 0;
  const run = (executable, args, tag) => {
    const result = spawnSync(executable, args, { cwd: scratch, encoding: 'utf8', timeout: 60000, windowsHide: true });
    const receipt = { executable, args, status: result.status, signal: result.signal, error: result.error?.message,
      stdout: result.stdout, stderr: result.stderr };
    fs.writeFileSync(path.join(evidence, `${sequence++}-${tag}.json`), JSON.stringify(receipt, null, 2));
    assert.ifError(result.error);
    assert.equal(result.status, 0, `${tag}: ${result.stdout}\n${result.stderr}`);
    return result.stdout;
  };
  run('powershell.exe', ['-NoProfile', '-File', io, manifest, 'create'], 'create-fixtures');
  const before = new Map();
  for (const item of scenarios) for (const link of item.links) if (fs.existsSync(link)) before.set(link, hash(fs.readFileSync(link)));
  for (const folder of ['', 'Tools']) {
    const selected = scenarios.filter(item => item.folder === folder);
    const output = path.join(scratch, `shortcut-${folder || 'root'}.exe`);
    const section = selected.map(item => {
      const update = item.mode === 'update' || item.mode === 'wix' ? 1 : 0;
      const noShortcut = item.mode === 'no-shortcuts' || item.mode === 'wix' ? 1 : 0;
      return `StrCpy $INSTDIR "${nsisString(item.install)}"
StrCpy $FixtureDesktop "${nsisString(item.desktop)}"
StrCpy $FixtureStartMenu "${nsisString(item.menu)}"
StrCpy $AppStartMenuFolder "${folder}"
StrCpy $OldMainBinaryName "opendiy-desktop.exe"
StrCpy $WixMode ${item.mode === 'wix' ? 1 : 0}
StrCpy $UpdateMode ${update}
StrCpy $NoShortcutMode ${noShortcut}
Call CreateOrUpdateStartMenuShortcut
${item.mode === 'finish' ? '' : 'Call CreateOrUpdateDesktopShortcut'}
!insertmacro NSIS_HOOK_POSTINSTALL
${item.mode === 'finish' ? 'Call CreateOrUpdateDesktopShortcut' : ''}
FileOpen $9 "${nsisString(path.join(item.root, 'completed'))}" w
FileWrite $9 "returned"
FileClose $9`;
    }).join('\n');
    const harness = `Unicode true
RequestExecutionLevel user
SilentInstall silent
OutFile "${nsisString(output)}"
!include LogicLib.nsh
!include FileFunc.nsh
!include "Win\\COM.nsh"
!include "Win\\Propkey.nsh"
!include "${nsisString(utils)}"
!include "${nsisString(path.join(scratch, 'hooks.nsh'))}"
!define PRODUCTNAME "Arcvera-Studio"
!define MAINBINARYNAME "arcvera-studio"
!define BUNDLEID "io.agentgrid.desktop"
!define STARTMENUFOLDER "${folder}"
Var FixtureDesktop
Var FixtureStartMenu
Var AppStartMenuFolder
Var OldMainBinaryName
Var WixMode
Var UpdateMode
Var NoShortcutMode
${functions}
Section
${section}
SectionEnd
`;
    const harnessPath = path.join(scratch, `shortcut-${folder || 'root'}.nsi`);
    fs.writeFileSync(harnessPath, harness);
    fs.copyFileSync(harnessPath, path.join(evidence, path.basename(harnessPath)));
    run(compiler, ['/V2', harnessPath], `compile-${folder || 'root'}`);
    run(output, ['/S'], `execute-${folder || 'root'}`);
  }
  const observed = JSON.parse(run('powershell.exe', ['-NoProfile', '-File', io, manifest, 'read'], 'read-fixtures'));
  fs.writeFileSync(path.join(evidence, 'observed.json'), JSON.stringify(observed, null, 2));
  const violations = [];
  const check = (condition, message) => { if (!condition) violations.push(message); };
  const samePath = (left, right) => left?.toLowerCase() === right.toLowerCase();
  for (const item of scenarios) {
    check(fs.existsSync(path.join(item.root, 'completed')), `${item.id}: production functions did not all return`);
    const actual = observed.find(row => row.id === item.id);
    assert.ok(actual, item.id);
    for (const link of item.links) {
      const row = actual.links.find(row => row.path === link);
      const foreign = ['foreign', 'corrupt', 'foreign-legacy'].includes(item.state);
      if (foreign) {
        check(row.exists && hash(fs.readFileSync(link)) === before.get(link), `${item.id}: foreign/unreadable shortcut bytes changed: ${link}`);
      } else {
        // Existing owned links migrate even in update/no-shortcut mode.
        const unconfiguredRoot = item.folder && link === path.join(item.menu, 'Arcvera-Studio.lnk');
        const mayCreate = !['update', 'no-shortcuts'].includes(item.mode) && !unconfiguredRoot;
        const legacyDestination = link === path.join(item.desktop, 'Arcvera-Studio.lnk') || link === path.join(item.menu, 'Arcvera-Studio.lnk');
        const expected = item.state === 'absent' ? mayCreate : item.state === 'legacy-only' ? mayCreate || legacyDestination : true;
        check(row.exists === expected, `${item.id}: unexpected shortcut existence: ${link}`);
        if (expected) check(samePath(row.target, path.join(item.install, 'arcvera-studio.exe')), `${item.id}: incorrect owned target: ${link}`);
        if (['current', 'recorded-old', 'current-legacy'].includes(item.state)) check(row.arguments === '--keep-user-arguments', `${item.id}: lost custom arguments`);
      }
    }
    for (const link of item.legacy) {
      const row = actual.links.find(row => row.path === link);
      const retained = item.state === 'foreign-legacy';
      check(row.exists === retained, `${item.id}: unexpected legacy shortcut existence`);
      if (retained) {
        check(samePath(row.target, path.join(item.install, 'arcvera-studio.exe')), `${item.id}: retained legacy target was not migrated`);
        check(row.arguments === '--keep-legacy-arguments', `${item.id}: lost retained legacy arguments`);
      }
    }
  }
  const report = { source, templateSha256: hash(Buffer.from(template)), hooksSha256: hash(Buffer.from(hooks)),
    testSha256: hash(fs.readFileSync(fileURLToPath(import.meta.url))), scenarioCount: scenarios.length, violations,
    boundary: 'Real NSIS function bodies and COM shortcuts; shell paths redirected. Actual installer/MUI/registry NOT_RUN.' };
  fs.writeFileSync(path.join(evidence, 'result.json'), JSON.stringify(report, null, 2));
  assert.deepEqual(violations, [], 'SC-ALL shortcut preservation/migration violations');
});
