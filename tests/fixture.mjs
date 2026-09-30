import { execFileSync, spawn } from 'node:child_process';
import { mkdirSync, writeFileSync, mkdtempSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join, resolve } from 'node:path';
export function createFixture() {
  const root = mkdtempSync(join(tmpdir(), 'xfold-test-'));
  const repo = join(root, 'repository'); mkdirSync(repo);
  const git = (...args) => execFileSync('git', ['-C', repo, ...args], { encoding: 'utf8', env: { ...process.env, GIT_AUTHOR_NAME: 'Xfold fixture', GIT_AUTHOR_EMAIL: 'test@localhost', GIT_COMMITTER_NAME: 'Xfold fixture', GIT_COMMITTER_EMAIL: 'test@localhost' } }).trim();
  git('init', '-q');
  const file = (path, text) => { const target = join(repo, path); mkdirSync(join(target, '..'), { recursive: true }); writeFileSync(target, text); };
  file('src/render.py', 'from .layout import arrange\n\ndef render(data):\n    return arrange(data)\n\ndef obsolete():\n    return "legacy"\n');
  file('src/layout.py', 'def arrange(data):\n    return sorted(data)\n');
  file('src/__init__.py', '');
  file('README.md', '# Graph fixture\n\nCommitted source for Xfold.\n');
  git('add', '.'); git('commit', '-qm', 'Initial semantic modules'); const base = git('rev-parse', 'HEAD');
  file('src/render.py', 'from .layout import arrange\n\ndef render(data):\n    return arrange(list(data))\n\ndef export_svg(data):\n    return render(data)\n');
  file('src/theme.py', 'def palette():\n    return ["red", "blue"]\n');
  git('add', '.'); git('commit', '-qm', 'Update rendering and add export'); const target = git('rev-parse', 'HEAD');
  // Deliberate dirty source must never be served by the application.
  file('src/render.py', '# UNCOMMITTED SENTINEL\n');
  return { root, repo, base, target, git, clean: () => rmSync(root, { recursive: true, force: true }) };
}
export async function startServer(fixture, extra = [], compare = true) {
  const executable = process.env.XFOLD_BINARY || resolve('target/release/xfold');
  const child = spawn(executable, [fixture.repo, ...(compare ? ['--from', fixture.base, '--to', fixture.target] : ['--ref', fixture.target]), '--cache-dir', join(fixture.root, 'cache'), '--no-open', ...extra], { stdio: ['ignore', 'pipe', 'pipe'] });
  let logs = ''; child.stderr.on('data', d => { logs += d; });
  const url = await new Promise((ok, reject) => {
    const timeout = setTimeout(() => reject(new Error(`Server did not start: ${logs}`)), 30000);
    child.stdout.on('data', d => { const match = String(d).match(/http:\/\/127\.0\.0\.1:\d+/); if (match) { clearTimeout(timeout); ok(match[0]); } });
    child.once('exit', code => { clearTimeout(timeout); reject(new Error(`Server exited ${code}: ${logs}`)); });
    child.once('error', reject);
  });
  return { child, url, logs: () => logs, stop: async () => { if (child.exitCode === null) { child.kill('SIGINT'); await new Promise(resolve => { child.once('exit', resolve); setTimeout(() => { child.kill('SIGKILL'); resolve(); }, 5000).unref(); }); } } };
}
export async function waitForArtifact(url, initial) {
  if (initial.state === 'ready') return initial;
  for (let i = 0; i < 300; i++) {
    const job = await (await fetch(`${url}/api/v1/jobs/${initial.id}`)).json();
    if (job.state === 'failed' || job.state === 'cancelled') throw new Error(JSON.stringify(job));
    if (job.state === 'ready') return { id: job.resultId, kind: job.resultKind, state: 'ready' };
    await new Promise(r => setTimeout(r, 100));
  }
  throw new Error('Timed out waiting for build');
}
