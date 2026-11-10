import assert from 'node:assert/strict';
import fs from 'node:fs';
import test from 'node:test';

const text = file => fs.readFileSync(new URL('../' + file, import.meta.url), 'utf8');
const json = file => JSON.parse(text(file));

test('Arcvera-Studio package, executable, application and version identities agree', () => {
  const pkg = json('package.json'), lock = json('package-lock.json');
  const config = json('src-tauri/tauri.conf.json'), cargo = text('src-tauri/Cargo.toml');
  assert.equal(pkg.name, 'arcvera-studio');
  assert.equal(lock.name, pkg.name);
  assert.equal(lock.packages[''].name, pkg.name);
  assert.equal(config.productName, 'Arcvera-Studio');
  assert.equal(config.app.windows[0].title, 'Arcvera-Studio');
  assert.equal(config.version, pkg.version);
  assert.match(cargo, new RegExp(`version = "${pkg.version.replaceAll('.', '\\.')}"`));
  assert.match(cargo, /default-run = "arcvera-studio"/);
  assert.match(cargo, /name = "arcvera_studio_lib"/);
  assert.match(text('src-tauri/tests/native_pty.rs'), /CARGO_BIN_EXE_arcvera-studio/);
  assert.match(text('index.html'), /<title>Arcvera-Studio<\/title>/);
});

test('CI packages and updater links use the renamed repository and binaries', () => {
  const workflow = text('.github/workflows/windows.yml');
  assert.match(workflow, /release\/arcvera-studio\.exe/);
  assert.match(workflow, /artifacts\/arcvera-studio\.exe/);
  assert.match(workflow, /name: arcvera-studio-windows-x64/);
  assert.match(workflow, /Arcvera-Studio-\$gridVersion-windows-x64-setup\.exe/);
  assert.match(text('scripts/updater-manifest.mjs'), /jaeseongs95\/Arcvera-Studio/);
  for (const file of ['README.md', 'README.ko.md']) {
    assert.match(text(file), /^# Arcvera-Studio/m);
    assert.match(text(file), /jaeseongs95\/Arcvera-Studio\/releases\/latest/);
    assert.doesNotMatch(text(file), /opendiy-desktop\.exe|github\.com\/jaeseongs95\/opendiy-desktop/);
  }
});

test('rebranding preserves the installed data identity and NSIS upgrade records', () => {
  const config = json('src-tauri/tauri.conf.json');
  assert.equal(config.identifier, 'io.agentgrid.desktop');
  assert.equal(config.bundle.windows.nsis.template, 'installer/installer.nsi');
  const template = text('src-tauri/installer/installer.nsi');
  assert.match(template, /!define UNINSTKEY "Software\\Microsoft\\Windows\\CurrentVersion\\Uninstall\\OpenDIY Desktop"/);
  assert.match(template, /!define MANUPRODUCTKEY "Software\\agentgrid\\OpenDIY Desktop"/);
  assert.match(template, /ReadRegStr \$4 SHCTX "\$\{MANUPRODUCTKEY\}" ""/);
  assert.match(template, /WriteRegStr SHCTX "\$\{UNINSTKEY\}" "DisplayName" "\$\{PRODUCTNAME\}"/);
});
