import base from '../../playwright.config';
import { fileURLToPath } from 'node:url';

export default {
  ...base,
  testDir: '../../tests',
  workers: 1,
  retries: 0,
  outputDir: `../../artifacts/korean-input-20261117/${process.env.ARCVERA_IME_BASELINE === '1' ? 'before' : 'after'}/results`,
  webServer: { cwd: fileURLToPath(new URL('../../', import.meta.url)), command: 'node verification/korean-input-20261117/serve.mjs', url: 'http://127.0.0.1:1420', reuseExistingServer: false },
};
