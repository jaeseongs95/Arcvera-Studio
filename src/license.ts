import { version } from '../package.json';
import licenseText from '../LICENSE?raw';
import { bindText } from './i18n';
import { openExternalUrl } from './external-links';

export const sourceRelease = `https://github.com/jaeseongs95/Arcvera-Studio/releases/tag/v${version}`;

/** Present the notice locally without requiring a network connection. */
export function createLicenseControl(onError: (error: unknown) => void) {
  const button = document.createElement('button');
  button.id = 'license-open'; button.type = 'button'; button.className = 'side-link';
  bindText(button, '라이선스·소스 · AGPL', 'License and source · AGPL');
  button.setAttribute('aria-haspopup', 'dialog'); button.setAttribute('aria-controls', 'license-dialog');
  const dialog = document.createElement('dialog'); dialog.id = 'license-dialog';
  dialog.setAttribute('aria-labelledby', 'license-title');
  dialog.style.cssText = 'width:min(680px,calc(100vw - 48px));max-height:80vh;overflow:auto';
  const title = document.createElement('h2'); title.id = 'license-title';
  title.textContent = `Arcvera-Studio ${version} · AGPL-3.0-or-later`;
  const notice = document.createElement('p');
  bindText(notice, 'Copyright (c) 2026 Agent Grid contributors. 무보증으로 제공됩니다. AGPL 제3판 또는 이후 판에 따라 수정·재배포할 수 있습니다.', 'Copyright (c) 2026 Agent Grid contributors. Provided without warranty. You may modify and redistribute under AGPL version 3 or later.');
  const source = document.createElement('a'); source.id = 'license-source'; source.href = sourceRelease;
  bindText(source, '이 버전의 대응 소스 무료 다운로드', 'Download this version’s Corresponding Source free of charge');
  source.onclick = event => { event.preventDefault(); void openExternalUrl(source.href).catch(onError); };
  const text = document.createElement('pre'); text.textContent = licenseText;
  text.style.cssText = 'white-space:pre-wrap;overflow-wrap:anywhere;font-size:12px;max-height:48vh;overflow:auto';
  const close = document.createElement('button'); close.type = 'button'; close.className = 'quiet-button';
  bindText(close, '닫기', 'Close'); close.onclick = () => dialog.close();
  const heading = document.createElement('div'); heading.className = 'dialog-heading'; heading.append(title, close);
  dialog.append(heading, notice, source, text); document.body.append(dialog);
  button.onclick = () => { dialog.showModal(); source.focus(); };
  dialog.addEventListener('close', () => button.focus());
  return button;
}
