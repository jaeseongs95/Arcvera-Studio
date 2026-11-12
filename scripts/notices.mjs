import fs from 'node:fs';
import path from 'node:path';
import { execFileSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';

// Package notices come from the exact downloaded sources used by this build.
const parts = ['# Third-party notices', 'Generated from the installed dependency sources. Arcvera-Studio 2.0.0 and later are AGPL-3.0-or-later licensed. Earlier MIT notices are preserved in LICENSE-MIT-LEGACY. Third-party licenses remain unchanged.'];
parts.push('## Tauri NSIS installer template 2.12.4\n\nLicense: MIT\n\n' + fs.readFileSync(fileURLToPath(new URL('../src-tauri/installer/TAURI-LICENSE-MIT', import.meta.url)), 'utf8'));
function include(name, version, license, directory) {
  parts.push(`## ${name} ${version}\n\nLicense: ${license || 'See package source'}`);
  const files = fs.readdirSync(directory).filter(name => /^(licen[cs]e|copying|notice)([._-]|$)/i.test(name));
  for (const file of files) {
    const absolute = path.join(directory, file);
    if (fs.statSync(absolute).isFile()) parts.push(`### ${file}\n\n\`\`\`text\n${fs.readFileSync(absolute, 'utf8')}\n\`\`\``);
  }
}
const lock = JSON.parse(fs.readFileSync('package-lock.json', 'utf8'));
for (const [location] of Object.entries(lock.packages)) {
  if (!location) continue;
  const manifest = path.join(location, 'package.json');
  if (!fs.existsSync(manifest)) continue; // Optional dependencies for another OS.
  const pkg = JSON.parse(fs.readFileSync(manifest, 'utf8'));
  include(pkg.name, pkg.version, pkg.license, location);
}
const metadata = JSON.parse(execFileSync('cargo', ['metadata', '--locked', '--format-version', '1', '--manifest-path', 'src-tauri/Cargo.toml'], { encoding: 'utf8', maxBuffer: 32 * 1024 * 1024 }));
for (const pkg of metadata.packages) {
  if (!pkg.source && !['portable-pty', 'tauri-plugin-updater'].includes(pkg.name)) continue;
  include(pkg.name, pkg.version, pkg.license, path.dirname(pkg.manifest_path));
}
fs.writeFileSync('THIRD_PARTY_NOTICES.md', parts.join('\n\n') + '\n', 'utf8');
