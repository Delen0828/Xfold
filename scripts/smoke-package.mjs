// Install the packed distribution into an empty prefix/cache, then build with
// no Rust, sem, Git executable, or source checkout available on PATH.
import { spawnSync } from 'node:child_process';
import { mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { join, resolve } from 'node:path';
import { tmpdir } from 'node:os';
import { createFixture } from '../tests/fixture.mjs';
const tarball = resolve(process.argv[2] || 'artifacts/xfold-cli-0.1.0.tgz');
const dir = mkdtempSync(join(tmpdir(), 'xfold-install-'));
const fixture = createFixture();
const report = { platform: `${process.platform}-${process.arch}`, node: process.version, tarball, offlineInstall: false, emptyPathRuntime: false };
try {
  const prefix = join(dir, 'install'); mkdirSync(prefix);
  writeFileSync(join(prefix, 'package.json'), '{"private":true}\n');
  const npm = spawnSync(process.platform === 'win32' ? 'npm.cmd' : 'npm', ['install', '--offline', '--ignore-scripts', '--omit=optional', '--no-audit', '--no-fund', '--cache', join(dir, 'npm-cache'), '--prefix', prefix, tarball], { encoding: 'utf8', shell: process.platform === 'win32' });
  if (npm.status !== 0) throw new Error(npm.stderr || npm.error?.message); report.offlineInstall = true;
  const launcher = join(prefix, 'node_modules', '@xfold', 'cli', 'bin', 'xfold.mjs');
  const bin = process.platform === 'win32' ? process.execPath : join(prefix, 'node_modules', '.bin', 'xfold');
  const version = spawnSync(bin, process.platform === 'win32' ? [launcher, '--version'] : ['--version'], { encoding: 'utf8', env: { ...process.env, XFOLD_BINARY: '' } });
  if (version.status !== 0) throw new Error(version.stderr || version.error?.message);
  report.packagedBinVersion = version.stdout.trim();
  const emptyPath = join(dir, 'empty-path'); mkdirSync(emptyPath);
  const env = { ...process.env, PATH: emptyPath, XFOLD_BINARY: '', HTTP_PROXY: 'http://127.0.0.1:9', HTTPS_PROXY: 'http://127.0.0.1:9' };
  const args = [launcher, 'build', fixture.repo, '--ref', fixture.target, '--cache-dir', join(dir, 'cache'), '--json'];
  const run = () => { const result = spawnSync(process.execPath, args, { encoding: 'utf8', env }); if (result.status !== 0) throw new Error(result.stderr || result.error?.message); return JSON.parse(result.stdout); };
  report.cold = run(); report.cached = run();
  if (!report.cold.validated || report.cold.graphHash !== report.cached.graphHash || report.cached.stats.filesParsed !== 0 || report.cached.stats.mode !== 'cache-hit') throw new Error('Invalid packaged build or cache hit');
  report.emptyPathRuntime = true; report.ok = true;
} catch (error) { report.ok = false; report.error = String(error); process.exitCode = 1; }
finally {
  fixture.clean(); rmSync(dir, { recursive: true, force: true });
  mkdirSync('artifacts', { recursive: true }); writeFileSync('artifacts/package-smoke.json', JSON.stringify(report, null, 2) + '\n');
  console.log(JSON.stringify(report, null, 2));
}
