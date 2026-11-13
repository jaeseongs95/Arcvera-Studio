import fs from 'node:fs';
import assert from 'node:assert/strict';

const pkg = JSON.parse(fs.readFileSync('package.json'));
const lock = JSON.parse(fs.readFileSync('package-lock.json'));
assert.equal(pkg.license, 'AGPL-3.0-or-later', 'Version 2.0.0 and every later release must retain AGPL');
assert.equal(lock.packages[''].license, pkg.license);
assert.equal(lock.packages[''].version, pkg.version);
assert.match(fs.readFileSync('src-tauri/Cargo.toml', 'utf8'), /^license = "AGPL-3\.0-or-later"$/m);
assert.match(fs.readFileSync('LICENSE', 'utf8'), /GNU AFFERO GENERAL PUBLIC LICENSE[\s\S]*Remote Network Interaction/);
const legacy = fs.readFileSync('LICENSE-MIT-LEGACY', 'utf8');
assert.match(legacy, /MIT License[\s\S]*Copyright \(c\) 2026 Agent Grid contributors[\s\S]*Permission is hereby granted[\s\S]*SOFTWARE\./);
for (const config of ['src-tauri/tauri.conf.json', 'src-tauri/tauri.bundle.conf.json']) {
  const resources = JSON.parse(fs.readFileSync(config)).bundle.resources;
  for (const file of ['LICENSE', 'LICENSE-MIT-LEGACY', 'THIRD_PARTY_NOTICES.md', 'SOURCE.md']) assert.equal(resources[`../${file}`], file);
}
for (const file of ['README.md', 'README.ko.md', 'SOURCE.md', 'AGENTS.md', 'scripts/notices.mjs']) assert.match(fs.readFileSync(file, 'utf8'), /AGPL-3\.0-or-later/);
assert.match(fs.readFileSync('THIRD_PARTY_NOTICES.md', 'utf8').split('\n').slice(0, 4).join('\n'), /AGPL-3\.0-or-later/);
console.log(`AGPL metadata and bundled notices agree for ${pkg.version}; legacy MIT preserved`);
