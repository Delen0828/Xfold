import { test, expect } from '@playwright/test';
// @ts-ignore Node fixture harness intentionally also runs outside TypeScript.
import { createFixture, startServer, waitForArtifact } from '../fixture.mjs';
import { mkdirSync, writeFileSync } from 'node:fs';
import { join } from 'node:path';

let fixture: any, server: any, comparison: any;
test.beforeAll(async () => { fixture = createFixture(); server = await startServer(fixture); const workspace = await (await fetch(`${server.url}/api/v1/workspace`)).json(); const ready = await waitForArtifact(server.url, workspace.initial); comparison = await (await fetch(`${server.url}/api/v1/comparisons/${ready.id}`)).json(); });
test.afterAll(async () => { await server?.stop(); fixture?.clean(); });

test('overview, drill-down, semantic diff, deleted source, and offline assets', async ({ page }) => {
  const errors: string[] = []; const remote: string[] = [];
  page.on('pageerror', e => errors.push(e.message));
  page.on('request', r => { if (!r.url().startsWith(server.url) && !r.url().startsWith('data:') && !r.url().startsWith('blob:')) remote.push(r.url()); });
  await page.goto(server.url);
  await expect(page.locator('.semantic-node')).toHaveCount(2);
  await expect(page.getByRole('region', { name: 'Semantic graph' })).toBeVisible();
  const directory = page.locator('.react-flow__node').filter({ has: page.locator('.node-name').filter({ hasText: /^src$/ }) });
  await directory.focus(); await page.keyboard.press('Enter'); await expect(page.locator('.source-heading h2')).toHaveText('src');
  await page.keyboard.press('Alt+Enter');
  await expect(page.locator('.semantic-node')).toHaveCount(4);
  await page.getByRole('button', { name: 'Enter render.py', exact: true }).click();
  await expect(page.locator('.node-name').filter({ hasText: /^render$/ })).toBeVisible();
  await page.locator('.node-name').filter({ hasText: /^render$/ }).click();
  await expect(page.locator('.source-heading h2')).toHaveText('render');
  await expect(page.locator('.monaco-diff-editor')).toBeVisible();
  await expect(page.locator('.monaco-diff-editor .view-lines').first()).toContainText('def');
  await expect(page.locator('.semantic-node').filter({ has: page.locator('.node-name').filter({ hasText: /^export_svg$/ }) })).toHaveClass(/added/);
  await expect(page.locator('.semantic-node').filter({ has: page.locator('.node-name').filter({ hasText: /^obsolete$/ }) })).toHaveClass(/deleted/);
  await page.locator('.monaco-diff-editor .view-line').filter({ hasText: 'def export_svg' }).last().click({ modifiers: ['ControlOrMeta'] });
  await expect(page.locator('.source-heading h2')).toHaveText('export_svg');
  await page.getByRole('button', { name: 'Toggle side-by-side source diff' }).click();
  expect((await page.locator('.code-pane').boundingBox())!.width).toBeGreaterThan(900);
  await page.getByRole('button', { name: 'Toggle side-by-side source diff' }).click();
  await page.locator('.node-name').filter({ hasText: /^obsolete$/ }).click();
  await page.getByRole('tab', { name: 'Before', exact: true }).click();
  await expect(page.locator('.monaco-editor .view-lines').first()).toContainText('obsolete');
  await page.getByRole('tab', { name: 'After', exact: true }).click();
  await expect(page.getByText('This module does not exist in the target commit.')).toBeVisible();
  await page.getByRole('tab', { name: 'Changes', exact: true }).click();
  await page.locator('.node-name').filter({ hasText: /^render$/ }).click();
  await page.getByRole('button', { name: 'Toggle minimap' }).click();
  await expect(page.locator('.react-flow__minimap')).toBeVisible();
  mkdirSync('artifacts', { recursive: true }); await page.screenshot({ path: 'artifacts/xfold-comparison.png', fullPage: true });
  expect(remote).toEqual([]); expect(errors).toEqual([]);
});

test('search, source coordinates, deep links, reduced motion, and keyboard navigation', async ({ page }) => {
  await page.emulateMedia({ reducedMotion: 'reduce' });
  await page.goto(server.url);
  await expect(page.locator('.semantic-node')).toHaveCount(2);
  await page.keyboard.press('/');
  const search = page.getByRole('textbox', { name: 'Find module' }); await expect(search).toBeFocused();
  await search.fill('arrange'); await page.getByRole('option').filter({ hasText: 'arrange' }).click();
  await expect(page.locator('.source-heading h2')).toHaveText('arrange');
  await expect(page.locator('.app')).toHaveClass(/still/);
  const link = page.url(); expect(link).toContain('commit='); expect(link).toContain('node='); expect(link).toContain('base=');
  await page.getByRole('combobox', { name: 'Base commit', exact: true }).fill('HEAD~9');
  expect(new URL(page.url()).hash).toBe(new URL(link).hash);
  await page.reload(); await expect(page.locator('.source-heading h2')).toHaveText('arrange');
  await page.getByRole('button', { name: 'Toggle theme' }).click(); await expect(page.locator('html')).toHaveAttribute('data-theme', 'light');
  await page.getByRole('button', { name: 'Dependencies →', exact: true }).click(); await expect(page.locator('.graph-inspector')).toContainText('Dependencies');
  await page.keyboard.press('Escape'); await expect(page.locator('.graph-inspector')).toHaveCount(0);
  const splitter = page.getByRole('separator', { name: 'Resize source pane' }); await splitter.focus(); await page.keyboard.press('ArrowLeft');
  await page.screenshot({ path: 'artifacts/xfold-light.png', fullPage: true });
});

test('API pins source to commits, expands aggregates, rejects foreign origins, and reports jobs', async ({ request }) => {
  const search = await (await request.get(`${server.url}/api/v1/snapshots/${comparison.target}/search?q=render&kind=function`)).json();
  const entity = search.items.find((m: any) => m.name === 'render'); expect(entity).toBeTruthy();
  const source = await (await request.get(`${server.url}/api/v1/snapshots/${comparison.target}/source?module=${entity.id}`)).json();
  expect(source.text).toContain('arrange(list(data))'); expect(source.text).not.toContain('UNCOMMITTED SENTINEL');
  const at = await (await request.get(`${server.url}/api/v1/snapshots/${comparison.target}/modules-at?path=src%2Frender.py&line=${source.anchor.startLine}`)).json(); expect(at.items[0].id).toBe(entity.id);
  const view = await (await request.post(`${server.url}/api/v1/graph-views`, { data: { comparison: comparison.id, scope: entity.parent } })).json();
  expect(view.nodes.some((n: any) => n.flags.includes('deleted'))).toBeTruthy();
  for (const edge of view.edges) { const expansion = await (await request.get(`${server.url}/api/v1/graph-views/${view.id}/edges/${edge.id}`)).json(); expect(expansion.items.length).toBe(edge.count); for (const relation of expansion.items) { expect(relation.source.anchor).toBeTruthy(); expect(relation.target.anchor).toBeTruthy(); } }
  expect((await request.post(`${server.url}/api/v1/snapshots`, { headers: { Origin: 'https://example.com' }, data: { revision: fixture.target } })).status()).toBe(403);
  expect((await request.get(`${server.url}/api/v1/snapshots/${comparison.target}/source?path=..%2F..%2Fetc%2Fpasswd`)).status()).toBe(400);
  const ready = await (await request.post(`${server.url}/api/v1/snapshots`, { data: { revision: fixture.target } })).json(); expect(ready.state).toBe('ready');
  const missing = await request.post(`${server.url}/api/v1/snapshots`, { data: { revision: 'does-not-exist' } }); expect(missing.status()).toBe(400);
  // Keep the worker busy with committed input, then cancel actual running work.
  // The fixture is private, and none of these Python functions are executed.
  mkdirSync(join(fixture.repo, 'slow'));
  for (let i = 0; i < 2000; i++) writeFileSync(join(fixture.repo, 'slow', `module_${i}.py`), `def function_${i}():\n    return ${i}\n`);
  fixture.git('add', 'slow'); fixture.git('commit', '-qm', 'Cancellation fixture');
  const forced = await (await request.post(`${server.url}/api/v1/snapshots`, { data: { revision: fixture.git('rev-parse', 'HEAD') } })).json();
  await expect.poll(async () => (await (await request.get(`${server.url}/api/v1/jobs/${forced.id}`)).json()).state, { timeout: 30000, intervals: [10, 20] }).toBe('running');
  const stream = await fetch(`${server.url}/api/v1/jobs/${forced.id}/events`);
  const reader = stream.body!.getReader(); const firstEvent = new TextDecoder().decode((await reader.read()).value);
  expect(firstEvent).toContain('event: job'); await reader.cancel();
  const cancelled = await (await request.post(`${server.url}/api/v1/jobs/${forced.id}/cancel`)).json(); expect(cancelled.state).toBe('cancelled');
  const preserved = await request.get(`${server.url}/api/v1/snapshots/${comparison.target}/source?module=${entity.id}`); expect(preserved.ok()).toBeTruthy();
  const restart = await (await request.post(`${server.url}/api/v1/snapshots`, { data: { revision: fixture.target, force: true } })).json(); await waitForArtifact(server.url, restart);
  const receipt = await (await request.get(`${server.url}/api/v1/snapshots/${comparison.target}/receipt`)).json(); expect(receipt.validated).toBe(true);
});
