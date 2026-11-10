import { copyFileSync, mkdirSync, writeFileSync } from 'node:fs';
import { resolve } from 'node:path';
import { get } from 'node:http';
import { execFile } from 'node:child_process';
import { promisify } from 'node:util';

const directory = resolve('artifacts/frontend-network');
const execute = promisify(execFile);

function probe() {
  return new Promise(resolveProbe => {
    const startedAt = new Date().toISOString();
    const request = get('http://127.0.0.1:1420/', { agent: false, signal: AbortSignal.timeout(2000) }, response => {
      let bytes = 0;
      response.on('data', chunk => { bytes += chunk.length; });
      response.on('end', () => resolveProbe({ startedAt, finishedAt: new Date().toISOString(), status: response.statusCode, bytes }));
      response.on('error', error => resolveProbe({ startedAt, error: error.message, code: error.code }));
    });
    request.on('error', error => resolveProbe({ startedAt, error: error.message, code: error.code }));
  });
}

async function snapshot() {
  // Aggregate inspection is confined to disposable GitHub Windows runners.
  if (process.env.GITHUB_ACTIONS !== 'true' || process.platform !== 'win32') return { status: 'NOT_RUN', reason: 'GitHub Windows runner only' };
  try {
    const { stdout } = await execute('pwsh', ['-NoProfile', '-NonInteractive', '-Command', `
      $ErrorActionPreference='Stop'
      $tcp=@(Get-NetTCPConnection -ErrorAction Stop)
      $os=Get-CimInstance Win32_OperatingSystem -ErrorAction Stop
      @{timeUtc=[DateTime]::UtcNow.ToString('o');tcp=@($tcp|Group-Object State|Select-Object Name,Count);loopback=@($tcp|Where-Object {$_.LocalPort -eq 1420 -or $_.RemotePort -eq 1420}|Group-Object State|Select-Object Name,Count);freePhysicalKB=$os.FreePhysicalMemory;freeVirtualKB=$os.FreeVirtualMemory;browser=@(Get-Process chrome,msedge -ErrorAction SilentlyContinue|Select-Object ProcessName,Id,Handles,WorkingSet64,PrivateMemorySize64)}|ConvertTo-Json -Depth 5 -Compress
    `], { timeout: 5000, windowsHide: true, maxBuffer: 1024 * 1024 });
    return JSON.parse(stdout);
  } catch (error) { return { measurementError: error.message }; }
}

export default class NetworkDiagnosticsReporter {
  failure = 0;
  pending = [];
  onBegin() { mkdirSync(directory, { recursive: true }); }
  onTestEnd(test, result) {
    if (result.status === test.expectedStatus || result.status === 'skipped') return;
    const index = ++this.failure;
    const observation = { title: test.titlePath(), status: result.status, expectedStatus: test.expectedStatus, retry: result.retry, timeUtc: new Date().toISOString() };
    // Finish before the next worker can overwrite the browser launch log.
    try { copyFileSync(resolve(directory, 'chromium-netlog.json'), resolve(directory, `failure-${index}-netlog.json`)); }
    catch (error) { observation.netlogError = error.message; }
    this.pending.push(Promise.all([probe(), snapshot()]).then(([http, host]) => {
      writeFileSync(resolve(directory, `failure-${index}.json`), JSON.stringify({ ...observation, http, host }, null, 2) + '\n');
    }).then(() => null, error => ({ measurementError: error.message })));
  }
  async onEnd(result) {
    const measurementErrors = (await Promise.all(this.pending)).filter(Boolean);
    writeFileSync(resolve(directory, 'suite-result.json'), JSON.stringify({ status: result.status, unexpectedResults: this.failure, measurementErrors, priorCause: 'UNCONFIRMED' }, null, 2) + '\n');
  }
}
