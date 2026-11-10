import fs from 'node:fs';
import path from 'node:path';
import crypto from 'node:crypto';
import { pathToFileURL } from 'node:url';

const REPO = 'jaeseongs95/Arcvera-Studio';
// Existing clients enforce this exact repository path before following GitHub's
// verified repository-rename redirect. New clients keep their own strict path.
const LEGACY_REPO = 'jaeseongs95/opendiy-desktop';

export function prepareUpdaterArtifacts({ projectRoot = process.cwd(), bundleDirectory, outputDirectory }) {
  const pkg = JSON.parse(fs.readFileSync(path.join(projectRoot, 'package.json'), 'utf8'));
  const config = JSON.parse(fs.readFileSync(path.join(projectRoot, 'src-tauri/tauri.conf.json'), 'utf8'));
  const version = pkg.version;
  if (!/^(0|[1-9]\d*)\.(0|[1-9]\d*)\.(0|[1-9]\d*)$/.test(version) || config.version !== version) {
    throw new Error('Updater packaging requires matching stable package/Tauri versions');
  }
  const expected = `${config.productName}_${version}_x64-setup.exe`;
  const installers = fs.readdirSync(bundleDirectory).filter(name => /\.exe$/i.test(name));
  if (installers.length !== 1 || installers[0] !== expected) {
    throw new Error('Expected exactly one NSIS installer for the current version and x64 target');
  }
  const installer = path.join(bundleDirectory, expected), signatureFile = installer + '.sig';
  if (!fs.lstatSync(installer).isFile() || fs.statSync(installer).size === 0 || !fs.lstatSync(signatureFile).isFile()) {
    throw new Error('Regular nonempty NSIS installer and matching .sig are required');
  }
  const signatureBytes = fs.readFileSync(signatureFile), signature = signatureBytes.toString('utf8').trim();
  if (!signature || signatureBytes.length > 16 * 1024 || signature.includes('\0')) {
    throw new Error('Missing or invalid updater signature content');
  }
  const name = `Arcvera-Studio-${version}-windows-x64-setup.exe`;
  const manifest = repo => ({ version, platforms: { 'windows-x86_64': {
    url: `https://github.com/${repo}/releases/download/v${version}/${name}`, signature,
  } } });
  if (fs.existsSync(outputDirectory)) throw new Error('Updater output already exists; preserve it and use a fresh directory');
  fs.mkdirSync(outputDirectory, { recursive: true });
  fs.copyFileSync(installer, path.join(outputDirectory, name), fs.constants.COPYFILE_EXCL);
  fs.copyFileSync(signatureFile, path.join(outputDirectory, name + '.sig'), fs.constants.COPYFILE_EXCL);
  fs.writeFileSync(path.join(outputDirectory, 'arcvera-latest.json'), JSON.stringify(manifest(REPO), null, 2) + '\n', { flag: 'wx' });
  fs.writeFileSync(path.join(outputDirectory, 'latest.json'), JSON.stringify(manifest(LEGACY_REPO), null, 2) + '\n', { flag: 'wx' });
  const sums = [name, name + '.sig', 'arcvera-latest.json', 'latest.json'].map(file => {
    const digest = crypto.createHash('sha256').update(fs.readFileSync(path.join(outputDirectory, file))).digest('hex');
    return `${digest}  ${file}`;
  });
  fs.writeFileSync(path.join(outputDirectory, 'SHA256SUMS.txt'), sums.join('\n') + '\n', { flag: 'wx' });
  return { version, installer: name, outputDirectory };
}

if (process.argv[1] && import.meta.url === pathToFileURL(path.resolve(process.argv[1])).href) {
  try {
    if (process.argv.length !== 4) throw new Error('Usage: node scripts/updater-manifest.mjs <NSIS directory> <fresh output directory>');
    const result = prepareUpdaterArtifacts({ bundleDirectory: process.argv[2], outputDirectory: process.argv[3] });
    console.log(`Prepared updater metadata for ${result.installer} in ${result.outputDirectory}; installation is not verified`);
  } catch (error) {
    console.error(error.message);
    process.exitCode = 1;
  }
}
