'use strict';
const fs=require('node:fs'),cp=require('node:child_process'),assert=require('node:assert/strict');
const base='b7fb7b2ba4f92ece17a62e69a643c90a8e62a4d2';
const prior=cp.execFileSync('git',['show',base+':src-tauri/src/external_links.rs'],{encoding:'utf8'});
let current=fs.readFileSync('src-tauri/src/external_links.rs','utf8');
const before='    #[cfg(feature = "test-link-observe")]\n    let observation = test_link_observe::begin(url);\n';
const after='    #[cfg(feature = "test-link-observe")]\n    if let Some(observation) = observation { observation.finish(result); }\n';
for(const addition of [before,after]){assert.equal(current.split(addition).length,2);current=current.replace(addition,'');}
assert.equal(current.slice(0,prior.length),prior,'Every original native source byte must remain; only two feature observations and appended TEST modules allowed');
const tail=current.slice(prior.length);assert(tail.startsWith('\n// TEST-only metadata; disabled in every normal product build.'));
assert(!/CoInitialize|CoUninitialize|spawn_blocking/.test(tail),'Observer must not acquire COM or add work');
const cargo=fs.readFileSync('src-tauri/Cargo.toml','utf8');assert(cargo.includes('test-link-observe = []'));assert(!/^default\s*=/m.test(cargo));
const frozen=cp.spawnSync('git',['diff','--exit-code',base,'HEAD','--','src','tests','playwright.config.ts','package.json','package-lock.json','.github/workflows/windows.yml'],{encoding:'utf8'});
assert.equal(frozen.status,0,'Frontend and regular validation must remain frozen: '+frozen.stdout+frozen.stderr);
const c=JSON.parse(fs.readFileSync('src-tauri/tauri.bundle.conf.json','utf8'));assert.equal(c.identifier,'io.agentgrid.desktop.test');assert.equal(c.productName,'OpenDIY Desktop TEST');assert.equal(c.app.windows[0].title,'OpenDIY Desktop TEST');
console.log(JSON.stringify({result:'PASS',base,originalNativeBytesPreserved:true,defaultFeatureOff:true,testIdentity:true}));
