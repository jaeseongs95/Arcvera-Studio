import { expect, test, type Page } from '@playwright/test';
import { appendFileSync, writeFileSync } from 'node:fs';
import { randomUUID } from 'node:crypto';

// Constant-sized fixture bookkeeping: retaining RPC arguments here would measure
// the test recorder's history instead of the application's display lifetime.
async function setup(page: Page, count = 1) {
  await page.goto('/');
  await page.evaluate(async count => {
    const { createConversation } = await import('/src/conversation.ts');
    const state = (window as any).resourceFixture = {
      generation: crypto.randomUUID(), sequence: 0, polls: 0, disposed: 0,
      controllers: [] as any[], hosts: [] as HTMLElement[], retired: [] as WeakRef<object>[],
      counts: { attach: 0, detach: 0, page: 0, items: 0 },
    };
    state.create = async (index: number) => {
      const host = document.createElement('section'); host.dataset.resourceIndex = String(index);
      host.style.cssText = 'position:fixed;inset:20px;background:#141820;z-index:9999';
      document.body.append(host);
      const controller = createConversation({ sessionId: `resource-${index}`, generation: 1, host,
        async call(command: string, args: any = {}) {
          if (command !== 'conversation_request') throw Error(`Unexpected ${command}`);
          const request = args.request;
          if (request.type in state.counts) state.counts[request.type]++;
          if (request.type === 'attach') return { threadId: `resource-thread-${index}`, activeTurnKnown: true, activeTurnId: null };
          if (request.type === 'detach') return {};
          if (request.type === 'models') return { data: [], nextCursor: null };
          if (request.type === 'page') return { data: [{ id: 'recent-turn', status: 'completed' }], nextCursor: null };
          if (request.type === 'items') {
            const revision = state.sequence++;
            return { data: Array.from({ length: 100 }, (_, item) => ({ turnId: 'recent-turn', item: {
              id: `history-${revision}-${item}`, type: 'agentMessage', text: `생성 대화 ${revision}/${item} ` + 'bounded history '.repeat(128),
            } })), nextCursor: null };
          }
          if (request.type === 'poll') { state.polls++; return { events: [], pendingRequests: [], connected: true, gap: false }; }
          throw Error(`Unexpected request ${request.type}`);
        },
      });
      state.controllers[index] = controller; state.hosts[index] = host;
      await controller.setForeground(false); await controller.setVisible(true);
    };
    state.retire = async (index: number) => {
      const controller = state.controllers[index], host = state.hosts[index];
      await controller.dispose(); host.remove();
      state.retired.push(new WeakRef(controller), new WeakRef(host)); state.disposed++;
      state.controllers[index] = null; state.hosts[index] = null;
    };
    for (let index = 0; index < count; index++) await state.create(index);
    await state.controllers[0].setForeground(true);
  }, count);
}

test('CR01 bounded history and foreground draft survive repeated replacements', async ({ page }) => {
  await setup(page);
  const host = page.locator('[data-resource-index="0"]');
  await host.locator('.conversation-input').fill('한글 초안 보존 CR01');
  for (let iteration = 0; iteration < 24; iteration++) {
    await page.evaluate(async () => { await (window as any).resourceFixture.controllers[0].refresh(); });
    await expect(host.locator('.conversation-message')).toHaveCount(100);
    await page.evaluate(async () => { await (window as any).resourceFixture.controllers[0].setForeground(false); });
    await expect(host.locator('.conversation-message')).toHaveCount(0);
    await expect(host.locator('.conversation-input')).toHaveValue('한글 초안 보존 CR01');
    await page.evaluate(async () => { await (window as any).resourceFixture.controllers[0].setForeground(true); });
    await expect(host.locator('.conversation-message')).toHaveCount(100);
  }
  const counts = await page.evaluate(() => (window as any).resourceFixture.counts);
  expect(counts.items).toBe(49);
  await page.evaluate(async () => { await (window as any).resourceFixture.retire(0); });
});

test('CR02 disposed controllers and hosts are collected and polling stops', async ({ page }, testInfo) => {
  if (process.env.RESOURCE_LIFETIME_MUTATION === 'listener') {
    await page.route('**/src/conversation.ts*', async route => {
      const response = await route.fetch(); const source = await response.text();
      // Require the actual production dispose boundary, never a test-only fake.
      const marker = 'abort.abort(); unlanguage();';
      expect(source.split(marker)).toHaveLength(2);
      await route.fulfill({ response, body: source.replace(marker, 'abort.abort(); /* sensitivity: omit language unsubscribe */') });
    });
  }
  await setup(page);
  for (let iteration = 0; iteration < 16; iteration++) {
    await page.evaluate(async iteration => {
      const state = (window as any).resourceFixture;
      await state.retire(0);
      if (iteration < 15) { await state.create(0); await state.controllers[0].setForeground(true); }
    }, iteration);
  }
  const cdp = await page.context().newCDPSession(page);
  await cdp.send('HeapProfiler.collectGarbage');
  // Every deref occurs in a later JS task than GC; WeakRef keep-alive does not
  // accidentally keep the target alive through the collection request.
  const retained = await page.evaluate(() => (window as any).resourceFixture.retired.filter((ref: WeakRef<object>) => ref.deref() !== undefined).length);
  await writeFileSync(testInfo.outputPath('collection.json'), JSON.stringify({ disposed: 16, expectedCollectedReferences: 32, retained }, null, 2));
  expect(retained, 'disposed controller/host retention').toBe(0);
  const polls = await page.evaluate(() => (window as any).resourceFixture.polls);
  // One full production polling period plus scheduling margin observes whether
  // disposal cancelled the timer, rather than merely checking the DOM is empty.
  await page.waitForTimeout(650);
  expect(await page.evaluate(() => (window as any).resourceFixture.polls)).toBe(polls);
});

test('CR03 sustained same-generation resource observations', async ({ page, browser }, testInfo) => {
  const seconds = Number(process.env.RESOURCE_LIFETIME_SOAK_SECONDS ?? 0);
  test.skip(seconds === 0, 'Opt-in sustained run; skipped is not T62 completion.');
  expect(Number.isInteger(seconds) && seconds >= 10 && seconds <= 14400, 'soak duration10..14400seconds').toBeTruthy();
  test.setTimeout((seconds + 120) * 1000);
  const runId = randomUUID(), startedAt = new Date().toISOString();
  const sampleMs = 5000, expectedSamples = Math.ceil(seconds * 1000 / sampleMs) + 1;
  const raw = testInfo.outputPath('resource-samples.jsonl');
  const summaryPath = testInfo.outputPath('resource-summary.json');
  const summary: Record<string, unknown> = {
    runId, startedAt, browserVersion: browser.version(), platform: process.platform,
    sourceCommit: process.env.GITHUB_SHA ?? null, scope: 'Chromium real conversation module; synthetic native RPC; not WebView/CLI/physicalIME or fullT62',
    workload: { sessions: 4, visibleItems: 100, sampleMs, requestedSeconds: seconds, pageReplacementsPerInterval: 1 },
    expectedSamples, observedSamples: 0, completed: false, missingMeasurements: [] as string[],
  };
  const append = (value: unknown) => appendFileSync(raw, JSON.stringify(value) + '\n');
  append({ kind: 'start', ...summary });
  let cdp: Awaited<ReturnType<ReturnType<Page['context']>['newCDPSession']>> | undefined;
  const started = performance.now();
  try {
    await setup(page, 4);
    cdp = await page.context().newCDPSession(page); await cdp.send('Performance.enable');
    const generation = await page.evaluate(() => (window as any).resourceFixture.generation);
    summary.generation = generation;
    for (let sample = 0; sample < expectedSamples; sample++) {
      const due = Math.min(sample * sampleMs, seconds * 1000);
      const wait = due - (performance.now() - started); if (wait > 0) await page.waitForTimeout(wait);
      const elapsedMs = performance.now() - started;
      const metrics = await cdp.send('Performance.getMetrics');
      const dom = await cdp.send('Memory.getDOMCounters');
      const metric = (name: string) => {
        const value = metrics.metrics.find(entry => entry.name === name)?.value;
        if (typeof value !== 'number' || !Number.isFinite(value)) {
          (summary.missingMeasurements as string[]).push(name); throw Error(`Missing metric ${name}`);
        }
        return value;
      };
      const state = await page.evaluate(() => {
        const state = (window as any).resourceFixture;
        return { generation: state.generation, polls: state.polls, sequence: state.sequence, displayedItems: document.querySelectorAll('[data-resource-index] .conversation-message').length, liveHosts: state.hosts.filter(Boolean).length };
      });
      const sampleValue = { kind: 'sample', runId, sample, elapsedMs, utc: new Date().toISOString(), heapUsedBytes: metric('JSHeapUsedSize'), heapTotalBytes: metric('JSHeapTotalSize'), taskDurationSeconds: metric('TaskDuration'), dom, ...state };
      append(sampleValue); summary.observedSamples = sample + 1;
      expect(state.generation).toBe(generation); expect(state.liveHosts).toBe(4); expect(state.displayedItems).toBe(100);
      expect(elapsedMs - due, 'sample scheduling gap').toBeLessThan(sampleMs);
      if (sample + 1 < expectedSamples) {
        await page.evaluate(async sample => {
          const state = (window as any).resourceFixture;
          const previous = sample % 4, next = (sample + 1) % 4;
          await state.controllers[previous].setForeground(false);
          await state.controllers[next].setForeground(true);
        }, sample);
      }
    }
    expect(summary.observedSamples).toBe(expectedSamples);
    expect(performance.now() - started).toBeGreaterThanOrEqual(seconds * 1000);
    summary.completed = true;
  } catch (error) {
    summary.error = error instanceof Error ? error.message : String(error); throw error;
  } finally {
    summary.finishedAt = new Date().toISOString(); summary.elapsedMs = performance.now() - started;
    append({ kind: 'finish', ...summary }); writeFileSync(summaryPath, JSON.stringify(summary, null, 2) + '\n');
    await cdp?.detach();
  }
});
