import { expect, test } from '@playwright/test';

test('실행 세대별 기능 관측을 교체하고 저장 메타데이터로 유출하지 않는다', async ({ page }) => {
  await page.goto('/');
  const result = await page.evaluate(async () => {
    const { updateSession, savedSessionMetadata, parseLayout } = await import('/src/model.ts');
    const { runtimeFeatures } = await import('/src/runtime-features.ts');
    const first = updateSession(undefined, { id: 'one', generation: 1, title: '세션', cwd: 'D:/Work', active: true, state: 'idle', runtimeFeatures: { conversation: true, manualCompact: true, tokenUsage: true, cron: true, subagents: true, remoteControl: true } });
    const observed = updateSession(first, { ...first, runtimeFeatures: { ...first.runtimeFeatures!, cron: false } });
    const restarted = updateSession(observed, { id: 'one', generation: 2, title: '세션', cwd: 'D:/Work', active: true, state: 'starting' });
    const stale = updateSession(restarted, first);
    return { observed: runtimeFeatures(observed), restarted: runtimeFeatures(restarted), staleIgnored: stale === restarted, saved: savedSessionMetadata(first), parsed: parseLayout({ version: 1, sessions: [first] })?.sessions };
  });
  expect(result.observed.cron).toBe(false);
  expect(Object.values(result.restarted)).toEqual([false, false, false, false, false, false]);
  expect(result.staleIgnored).toBe(true);
  expect(result.saved).toEqual({ title: '세션', cwd: 'D:/Work' });
  expect(result.parsed).toEqual([{ title: '세션', cwd: 'D:/Work' }]);
});
