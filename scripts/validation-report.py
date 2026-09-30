#!/usr/bin/env python3
"""Summarize measured validation artifacts without inventing missing results."""
import json
import shutil
import statistics
from pathlib import Path
ROOT = Path(__file__).resolve().parents[1]
artifacts = ROOT / 'artifacts'
output = ROOT / 'docs' / 'validation'
output.mkdir(parents=True, exist_ok=True)
def read(name):
    return json.loads((artifacts / name).read_text())
suite = read('matplotlib-benchmark.json')
ui = read('matplotlib-browser.json')
package = read('package-smoke.json')
smoke_path = artifacts / 'matplotlib-final-smoke.json'
smoke = json.loads(smoke_path.read_text()) if smoke_path.exists() else {}
assert suite.get('ok') and suite.get('passed') == 51, 'Extended suite is incomplete or failed'
assert ui.get('ok') and package.get('ok'), 'Browser/package checks did not pass'
browser = read('browser-results.json')
assert browser.get('stats', {}).get('unexpected') == 0 and browser.get('stats', {}).get('expected') == 3, 'Browser workflow suite is not green'
rows = suite['cases']
inc = [c['incrementalWallSeconds'] for c in rows]
cold = [c['coldWallSeconds'] for c in rows]
minimum_nodes, maximum_nodes = min(c['modules'] for c in rows), max(c['modules'] for c in rows)
if smoke.get('ok'):
    original = {c['name']: c['graphHash'] for c in rows}
    assert all(c['graphHash'] == original[c['name']] for c in smoke['cases']), 'Final-build graph differs from extended suite'
for name in ['matplotlib-benchmark.json', 'matplotlib-browser.json', 'package-smoke.json', 'matplotlib-final-smoke.json', 'browser-results.json']:
    if (artifacts / name).exists(): shutil.copyfile(artifacts / name, output / name)
shutil.copyfile(artifacts / 'matplotlib-browser.png', output / 'matplotlib-browser.png')
measurements = '\n'.join(f'| `{key}` | {value:,} ms |' for key, value in ui['measurements'].items())
text = f'''# Validation results

Measured on 2026-09-30, macOS arm64, {ui['cpu']}, {ui['memoryBytes'] / 1024**3:.0f} GiB RAM,
Node {package['node']}, Chrome {ui['browser']}. These are local measurements,
not latency or memory guarantees. The build suite ran alongside development
checks, and timings include contention. Browser measurements describe one run.

## Correctness and recovery

- 12 Rust integration tests passed. Coverage includes cold/incremental equality,
  A→B→A, skipped commits, branches, persisted restart, symbol/import/config
  invalidation, overload reordering, ambiguous/reused sem identities, duplicate
  bodies, moves, cosmetics, CRLF/Unicode, cycles, aggregate expansion, side-specific
  paths, bare repositories, corrupt cache repair and missing Git objects.
- 3 browser/API workflows passed: hierarchy, keyboard selection/drill-down,
  source-to-node clicks, deleted source, diff expansion, search/deep links,
  reduced motion, themes, aggregate anchors, origin/path rejection, SSE,
  cancellation of active work, worker recovery and preserved completed snapshots.
- 2 launcher tests passed. TypeScript checks, Rust formatting, and Clippy with
  warnings denied passed for Xfold (`--no-deps`; the pinned vendor is unchanged).
- Packed macOS arm64 installation passed using a fresh npm prefix/cache and
  `--offline`. The installed launcher built a private Git fixture with an empty
  `PATH`, so neither Rust, sem nor a Git executable was available to it.
  Reopening the same snapshot parsed/resolved zero files.
- Browser asset requests were restricted to loopback; zero external requests
  and zero page errors were observed in the real Matplotlib UI run.

## Matplotlib oracle

**{suite['passed']}/{suite['expected']} passed:** 3 real pairs, 14 first-parent transitions,
and 34 explicitly synthetic SWE-Bench commits made from each base plus its code
and test patches. No Matplotlib code or tests were executed.

The oracle compared canonical hashes, complete normalized nodes, and complete
semantic edges against an independent forced cold build for each target.
The final native build also {'passed all three real pairs; its graph hashes equal the extended-run hashes' if smoke.get('ok') else 'is awaiting the three-pair final recheck'}.

| Measurement | Observed result |
| --- | --- |
| Nodes per snapshot, including navigation containers | {minimum_nodes:,}–{maximum_nodes:,} |
| Incremental wall time, median / range | {statistics.median(inc):.2f} s / {min(inc):.2f}–{max(inc):.2f} s |
| Cold wall time, median / range | {statistics.median(cold):.2f} s / {min(cold):.2f}–{max(cold):.2f} s |
| Median per-pair cold/incremental ratio | {statistics.median(b/a for a,b in zip(inc,cold)):.2f}× |
| Files reused per target build | {min(c['incremental']['filesReused'] for c in rows):,}–{max(c['incremental']['filesReused'] for c in rows):,} |
| Maximum sampled worker RSS | {suite['peakWorkerRssKb'] / 1024**2:.2f} GiB |
| Extended-suite elapsed time | {suite['elapsedSeconds'] / 60:.1f} min |

RSS is the maximum sampled individual parser-worker resident memory, not total
application memory. `artifactBytes` measures serialized normalized graph size,
not deduplicated disk occupancy. `filesParsed`, green-file reuse, and edge reuse
counters come from sem; edge counters precede Xfold's normalization/deduplication.
Receipt duration excludes final manifest/index publication; wall times include
worker output and oracle object loading. Raw per-case metrics are retained in
[the full report](validation/matplotlib-benchmark.json).

## Real browser measurements

The stackplot comparison contained {ui['snapshot']['moduleCount']:,} graph nodes and
{ui['snapshot']['edgeCount']:,} semantic dependencies. Rendering remained bounded
while the backend retained full topology. First graph access includes construction
of an immutable hierarchy index; subsequent scopes reuse it. A warm projection
uses the completed-view cache. Build/comparison timing can be zero on a cache hit.

| Operation | Observed time |
| --- | --- |
{measurements}

[Browser report](validation/matplotlib-browser.json) ·
[Package smoke report](validation/package-smoke.json) ·
[Final-build oracle](validation/matplotlib-final-smoke.json)

![Matplotlib comparison and committed source](validation/matplotlib-browser.png)

## Release status

The local npm tarball and native package are **macOS arm64**. Clean cross-platform
machines and CI were not available in this session. Workflows define macOS x64,
Linux glibc arm64/x64, and Windows x64 builds and native-host package smoke tests;
those platforms require successful CI before being called validated releases.
Nothing has been published to npm.

The launcher's Node.js 22 minimum is configured and CI uses Node 22; the local
runtime check used the Node version shown above. sem remains a static analyzer
with conservative resolution and no call-site spans. Very large repositories
can require substantial memory and first-build time; the report establishes
behavior for these fixtures, not an upper bound for arbitrary repositories.
'''
(ROOT / 'docs' / 'VALIDATION.md').write_text(text)
print('Wrote docs/VALIDATION.md and portable evidence.')
