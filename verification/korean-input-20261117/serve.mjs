import { createServer } from 'vite';
import { execFileSync } from 'node:child_process';
import { resolve } from 'node:path';

// The shared Windows PC's filesystem watcher is expensive. This immutable
// single-run fixture needs no HMR; verify the final resolved option explicitly.
const server = await createServer({
  server: { host: '127.0.0.1', port: 1420, strictPort: true, hmr: false },
  plugins: [{
    name: 'immutable-ime-test', enforce: 'pre',
    configResolved(config) { config.server.watch = null; },
    load(id) {
      if (process.env.ARCVERA_IME_BASELINE === '1' && id.replaceAll('\\', '/') === resolve('src/main.ts').replaceAll('\\', '/')) {
        return execFileSync('git', ['show', 'a02913d17c371e5c4339ac81b6481a3ef595c4fa:src/main.ts'], { encoding: 'utf8' });
      }
    },
  }],
});
if (server.config.server.watch !== null) throw new Error('IME fixture watcher must be disabled');
await server.listen();
console.log('IME source fixture ready; filesystem watcher disabled');
for (const signal of ['SIGINT', 'SIGTERM']) process.once(signal, async () => { await server.close(); process.exit(0); });
