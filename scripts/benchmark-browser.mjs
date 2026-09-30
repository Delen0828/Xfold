// Browser/API measurements on a real Matplotlib comparison. No repository code runs.
import { chromium } from '@playwright/test';
import { performance } from 'node:perf_hooks';
import { mkdirSync, readFileSync, writeFileSync } from 'node:fs';
import { resolve } from 'node:path';
import { platform, arch, cpus, totalmem } from 'node:os';
import { startServer } from '../tests/fixture.mjs';
const pair = JSON.parse(readFileSync('fixtures/matplotlib.json')).pairs[0];
const root = resolve('.cache/matplotlib-browser'); mkdirSync(root, { recursive: true });
const repo = resolve(process.argv[2] || '../sem-test/cache/matplotlib__matplotlib.git');
const report = { platform: `${platform()}-${arch()}`, cpu: cpus()[0]?.model, memoryBytes: totalmem(), pair, measurements: {}, errors: [], remoteRequests: [] };
let server, browser;
async function timed(name, fn) { const start = performance.now(); const value = await fn(); report.measurements[name] = Math.round(performance.now() - start); return value; }
try {
  server = await startServer({ root, repo, base: pair.base, target: pair.target });
  const api = async (path, body) => { const response = await fetch(`${server.url}/api/v1${path}`, { method: body ? 'POST' : 'GET', body: body ? JSON.stringify(body) : undefined, headers: body ? { 'Content-Type': 'application/json' } : {} }); const data = await response.json(); if (!response.ok) throw new Error(JSON.stringify(data)); return data; };
  const workspace = await api('/workspace');
  let ready = workspace.initial;
  await timed('initialBuildAndComparisonMs', async () => {
    for (let i = 0; ready.state !== 'ready'; i++) {
      if (i > 2400) throw new Error('Build timeout');
      const job = await api(`/jobs/${ready.id}`);
      if (job.state === 'failed' || job.state === 'cancelled') throw new Error(JSON.stringify(job));
      if (job.state === 'ready') { ready = { id: job.resultId, state: 'ready' }; break; }
      if (i % 40 === 0) console.log(`Matplotlib: ${job.phase}`);
      await new Promise(r => setTimeout(r, 250));
    }
  });
  const comparison = await timed('comparisonLoadMs', () => api(`/comparisons/${ready.id}`));
  const snapshot = await api(`/snapshots/${comparison.target}`);
  report.snapshot = snapshot; report.receipt = await api(`/snapshots/${comparison.target}/receipt`);
  const search = await timed('searchMs', () => api(`/snapshots/${snapshot.id}/search?q=stackplot&kind=function`));
  const selected = search.items.find(m => m.path === 'lib/matplotlib/stackplot.py' && m.name === 'stackplot');
  if (!selected) throw new Error('Matplotlib stackplot entity missing');
  await timed('fileGraphViewMs', () => api('/graph-views', { comparison: comparison.id, scope: selected.parent, limit: 300 }));
  await timed('sourceDiffMs', () => api(`/comparisons/${comparison.id}/source-diff?module=${selected.id}`));
  browser = await chromium.launch({ channel: process.env.CI ? undefined : 'chrome' }); report.browser = browser.version();
  const context = await browser.newContext({ viewport: { width: 1512, height: 982 }, reducedMotion: 'reduce' });
  await context.route('**/*', route => { const url = route.request().url(); if (url.startsWith(server.url) || url.startsWith('data:') || url.startsWith('blob:')) return route.continue(); report.remoteRequests.push(url); return route.abort(); });
  const page = await context.newPage(); page.on('pageerror', e => report.errors.push(e.stack || e.message));
  await timed('browserOverviewReadyMs', async () => { await page.goto(server.url); await page.locator('.semantic-node').first().waitFor({ timeout: 60000 }); });
  await page.getByRole('textbox', { name: 'Find module' }).fill('stackplot');
  const option = page.getByRole('option').filter({ hasText: 'lib/matplotlib/stackplot.py' }).filter({ has: page.locator('strong').filter({ hasText: /^stackplot$/ }) }).first(); await option.waitFor();
  await timed('selectionToSourceMs', async () => { await option.click(); await page.locator('.monaco-diff-editor .view-lines').first().waitFor(); await page.waitForFunction(() => document.querySelector('.source-heading h2')?.textContent === 'stackplot'); });
  await page.locator('.node-name').filter({ hasText: /^stackplot$/ }).waitFor({ timeout: 60000 });
  await page.screenshot({ path: 'artifacts/matplotlib-browser.png' });
  report.deepLink = new URL(page.url()).hash;
  await timed('nextScopeGraphViewMs', () => api('/graph-views', { comparison: comparison.id, scope: 'root', expanded: [selected.parent], limit: 300 }));
  await timed('warmGraphViewMs', () => api('/graph-views', { comparison: comparison.id, scope: selected.parent, limit: 300 }));
  await timed('warmSourceDiffMs', () => api(`/comparisons/${comparison.id}/source-diff?module=${selected.id}`));
  await page.getByRole('tab', { name: 'Before', exact: true }).click(); await page.locator('.monaco-editor .view-lines').first().waitFor();
  await page.getByRole('tab', { name: 'After', exact: true }).click(); await page.locator('.monaco-editor .view-lines').first().waitFor();
  if (report.errors.length || report.remoteRequests.length) throw new Error('Browser or offline-asset errors');
  report.ok = true;
} catch (error) { report.ok = false; report.error = String(error); process.exitCode = 1; }
finally {
  await browser?.close(); await server?.stop();
  mkdirSync('artifacts', { recursive: true }); writeFileSync('artifacts/matplotlib-browser.json', JSON.stringify(report, null, 2) + '\n');
  console.log(JSON.stringify(report, null, 2));
}
