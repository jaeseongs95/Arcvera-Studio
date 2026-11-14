import { defineConfig } from '@playwright/test';
import { mkdirSync } from 'node:fs';
import { resolve } from 'node:path';

const bundled = process.env.AGENT_GRID_BUNDLED_FRONTEND_TEST === '1';
const sourceOnly = process.env.AGENT_GRID_SOURCE_FIXTURES_ONLY === '1';
if (bundled && sourceOnly) throw new Error('Choose bundled UI or source fixtures, not both.');
// Opt-in fixture observations; no retries or browser feature overrides.
const networkDiagnostics = process.env.AGENT_GRID_NETWORK_DIAGNOSTICS === '1';
const networkDirectory = resolve('artifacts/frontend-network');
if (networkDiagnostics) mkdirSync(networkDirectory, { recursive: true });

// These fixtures import source modules or intercept Vite module responses.
// Keep mixed files together so their fault-injection paths remain available.
const sourceFixtures = [
  'context-capabilities', 'context-controls', 'context-composition', 'conversation-integration',
  'conversation', 'cron-manager', 'external-links', 'link-drag',
  'project-management', 'project-switch-scroll', 'project-tabs',
  'question-center-limits', 'question-center', 'saved-cron', 'startup-runtime',
  'subagent-manager', 'terminal-flow', 'weekly-usage', 'permission-status', 'mcp-manager', 'mcp-runtime', 'hook-runtime', 'hook-runtime-manager', 'update-settings',
].map(name => `**/${name}.spec.ts`);

export default defineConfig({
  testDir: './tests',
  testMatch: sourceOnly ? sourceFixtures : undefined,
  testIgnore: bundled ? sourceFixtures : undefined,
  workers: 1,
  reporter: networkDiagnostics ? [['dot'], ['./tests/network-diagnostics-reporter.mjs']] : undefined,
  outputDir: process.env.AGENT_GRID_TEST_OUTPUT ?? 'test-results',
  use: {
    baseURL: 'http://127.0.0.1:1420', viewport: { width: 1440, height: 1000 },
    channel: process.env.PLAYWRIGHT_CHANNEL, trace: 'retain-on-failure',
    launchOptions: networkDiagnostics ? { args: [`--log-net-log=${resolve(networkDirectory, 'chromium-netlog.json')}`] } : undefined,
  },
  webServer: {
    command: bundled ? 'npm run preview -- --host 127.0.0.1 --port 1420 --strictPort' : 'npm run dev',
    url: 'http://127.0.0.1:1420', reuseExistingServer: !process.env.CI,
  },
});
