import { test } from 'node:test';
import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import { createFixture } from './fixture.mjs';
import { join, resolve } from 'node:path';
test('npm launcher exposes native help and rejects non-Git input', () => {
  const result = spawnSync(process.execPath, ['bin/xfold.mjs', '--help'], { encoding: 'utf8' });
  assert.equal(result.status, 0, result.stderr); assert.match(result.stdout, /semantic graphs/);
  const fixture = createFixture();
  try { const bad = spawnSync(process.execPath, ['bin/xfold.mjs', join(fixture.root, 'absent'), '--no-open'], { encoding: 'utf8' }); assert.notEqual(bad.status, 0); assert.match(bad.stderr, /not a Git repository/); } finally { fixture.clean(); }
});
test('headless build produces a verifiable receipt and a zero-analysis cache hit', () => {
  const fixture = createFixture();
  try {
    const args = ['bin/xfold.mjs', 'build', fixture.repo, '--ref', fixture.target, '--cache-dir', join(fixture.root, 'cache'), '--json'];
    const cold = spawnSync(process.execPath, args, { encoding: 'utf8' }); assert.equal(cold.status, 0, cold.stderr);
    const first = JSON.parse(cold.stdout); assert.equal(first.validated, true); assert.ok(first.stats.filesParsed > 0);
    const hit = spawnSync(process.execPath, args, { encoding: 'utf8' }); assert.equal(hit.status, 0, hit.stderr);
    const second = JSON.parse(hit.stdout); assert.equal(second.graphHash, first.graphHash); assert.equal(second.stats.mode, 'cache-hit'); assert.equal(second.stats.filesParsed, 0);
  } finally { fixture.clean(); }
});
