#!/usr/bin/env python3
"""Cold/incremental oracle over real and synthetic Matplotlib commits.

Uses private Git objects/index and private Xfold caches. Never executes analyzed
code, checks out a user repository, or alters sem-test's saved results.
"""
import argparse
import hashlib
import json
import os
import platform
import queue
import shutil
import subprocess
import sys
import tempfile
import threading
import time
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]


def run(*args, data=None, env=None):
    return subprocess.run(args, input=data, env=env, check=True, stdout=subprocess.PIPE, stderr=subprocess.PIPE).stdout.decode().strip()


class Worker:
    def __init__(self, binary, repo, cache, log):
        self.cache = cache
        cache.mkdir(parents=True, exist_ok=True)
        self.process = subprocess.Popen([str(binary), 'worker', '--repository', str(repo), '--cache-dir', str(cache)], stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=log, text=True, bufsize=1)
        self.lines = queue.Queue()
        self.peak_rss_kb = 0
        def read():
            for line in self.process.stdout:
                self.lines.put(line)
            self.lines.put(None)
        threading.Thread(target=read, daemon=True).start()
        def memory():
            while self.process.poll() is None:
                if os.name != 'nt':
                    try:
                        rss = int(run('ps', '-o', 'rss=', '-p', str(self.process.pid)))
                        self.peak_rss_kb = max(self.peak_rss_kb, rss)
                    except (ValueError, subprocess.CalledProcessError, OSError):
                        pass
                time.sleep(.25)
        threading.Thread(target=memory, daemon=True).start()

    def object(self, key):
        data = (self.cache / 'objects' / key[:2] / key[2:]).read_bytes()
        assert hashlib.sha256(data).hexdigest() == key, f'Bad object checksum: {key}'
        return json.loads(data)

    def build(self, revision, force=False):
        started = time.perf_counter()
        self.process.stdin.write(json.dumps({'revision': revision, 'force': force}) + '\n')
        self.process.stdin.flush()
        while True:
            line = self.lines.get(timeout=600)
            if line is None:
                raise RuntimeError(f'Worker exited: {self.process.poll()}')
            event = json.loads(line)
            if event.get('error'):
                raise RuntimeError(event['error'])
            if event.get('snapshot'):
                snapshot = self.object(event['snapshot'])
                if snapshot.get('format') == 'xfold/snapshot-chunks/1':
                    manifest = snapshot
                    snapshot = manifest['snapshot']
                    snapshot['modules'] = {m['id']: m for key in manifest['moduleChunks'].values() for m in self.object(key)}
                    snapshot['edges'] = sorted([e for key in manifest['edgeChunks'].values() for e in self.object(key)], key=lambda e: e['id'])
                receipt = self.object(event['receipt'])
                receipt['wallSeconds'] = time.perf_counter() - started
                return snapshot, receipt

    def close(self):
        if self.process.poll() is None:
            self.process.stdin.close()
            try:
                self.process.wait(timeout=20)
            except subprocess.TimeoutExpired:
                self.process.kill()
                self.process.wait()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--repo', type=Path, default=ROOT.parent / 'sem-test/cache/matplotlib__matplotlib.git')
    parser.add_argument('--cases', type=Path, default=ROOT.parent / 'sem-test/results/local-source-all')
    parser.add_argument('--binary', type=Path, default=ROOT / 'target/release/xfold')
    parser.add_argument('--output', type=Path, default=ROOT / 'artifacts/matplotlib-benchmark.json')
    parser.add_argument('--suite', choices=['smoke', 'history', 'synthetic', 'all'], default='all')
    args = parser.parse_args()
    manifest = json.loads((ROOT / 'fixtures/matplotlib.json').read_text())
    args.output.parent.mkdir(parents=True, exist_ok=True)
    report = {'engine': run(str(args.binary), '--version'), 'platform': platform.platform(), 'cpuCount': os.cpu_count(), 'suite': args.suite, 'repository': str(args.repo.resolve()), 'cases': [], 'failures': [], 'peakWorkerRssKb': 0}
    started = time.perf_counter()
    with tempfile.TemporaryDirectory(prefix='xfold-matplotlib-') as temporary:
        temp = Path(temporary)
        repo = temp / 'repository.git'
        run('git', 'clone', '--bare', '--shared', str(args.repo.resolve()), str(repo))
        env = dict(os.environ, GIT_INDEX_FILE=str(temp / 'index'), GIT_AUTHOR_NAME='Xfold fixture', GIT_AUTHOR_EMAIL='fixture@localhost', GIT_COMMITTER_NAME='Xfold fixture', GIT_COMMITTER_EMAIL='fixture@localhost', GIT_AUTHOR_DATE='2000-01-01T00:00:00Z', GIT_COMMITTER_DATE='2000-01-01T00:00:00Z')
        def git(*command, data=None):
            return run('git', f'--git-dir={repo}', *command, data=data, env=env)
        pairs = []
        if args.suite in ('smoke', 'all'):
            pairs += [{**p, 'synthetic': False} for p in manifest['pairs']]
        if args.suite in ('history', 'all'):
            seq = manifest['sequence']
            commits = git('rev-list', '--first-parent', f'--max-count={seq["transitions"]+1}', seq['end']).splitlines()[::-1]
            assert len(commits) == seq['transitions'] + 1
            pairs += [{'name': f'history-{i+1:02}', 'base': a, 'target': b, 'synthetic': False} for i, (a, b) in enumerate(zip(commits, commits[1:]))]
        if args.suite in ('synthetic', 'all'):
            cases = sorted(args.cases.glob(manifest['synthetic']['pattern']))
            assert len(cases) == manifest['synthetic']['expectedCount'], f'Expected 34 cases, found {len(cases)}'
            for path in cases:
                case = json.loads(path.read_text())
                git('read-tree', case['base_commit'])
                for field in ('patch', 'test_patch'):
                    if case.get(field):
                        git('apply', '--cached', '--whitespace=nowarn', '-', data=case[field].encode())
                tree = git('write-tree')
                target = git('commit-tree', tree, '-p', case['base_commit'], '-m', f'SYNTHETIC Xfold validation: {case["instance_id"]}')
                pairs.append({'name': case['instance_id'], 'base': case['base_commit'], 'target': target, 'synthetic': True})
        with args.output.with_suffix('.stderr.log').open('w') as log:
            warm = Worker(args.binary.resolve(), repo, temp / 'warm', log)
            cold = Worker(args.binary.resolve(), repo, temp / 'cold', log)
            current = None
            try:
                for i, pair in enumerate(pairs):
                    print(f'[{i+1}/{len(pairs)}] {pair["name"]}', flush=True)
                    try:
                        if current != pair['base']:
                            base, _ = warm.build(pair['base'])
                        incremental, inc = warm.build(pair['target'])
                        clean, fresh = cold.build(pair['target'], force=True)
                        assert incremental['graphHash'] == clean['graphHash'], 'Cold/incremental graph mismatch'
                        assert incremental['modules'] == clean['modules'], 'Entity mismatch'
                        assert incremental['edges'] == clean['edges'], 'Edge mismatch'
                        row = {**pair, 'graphHash': clean['graphHash'], 'modules': len(clean['modules']), 'edges': len(clean['edges']), 'incremental': inc['stats'], 'cold': fresh['stats'], 'incrementalWallSeconds': inc['wallSeconds'], 'coldWallSeconds': fresh['wallSeconds'], 'equal': True}
                        report['cases'].append(row)
                        current = pair['target']
                        print(f'  equal · {row["modules"]} modules · {inc["wallSeconds"]:.2f}s incremental / {fresh["wallSeconds"]:.2f}s cold · {inc["stats"]["filesReused"]} files reused', flush=True)
                    except Exception as error:
                        report['failures'].append({**pair, 'error': str(error)})
                        print(f'  FAILED: {error}', flush=True)
                        # A failing worker is restarted to keep failures independent.
                        warm.close(); cold.close()
                        warm = Worker(args.binary.resolve(), repo, temp / 'warm', log)
                        cold = Worker(args.binary.resolve(), repo, temp / 'cold', log)
                        current = None
                    report['elapsedSeconds'] = time.perf_counter() - started
                    report['peakWorkerRssKb'] = max(report['peakWorkerRssKb'], warm.peak_rss_kb, cold.peak_rss_kb)
                    args.output.write_text(json.dumps(report, indent=2) + '\n')
            finally:
                warm.close(); cold.close()
    report['elapsedSeconds'] = time.perf_counter() - started
    report['passed'] = len(report['cases'])
    report['expected'] = len(pairs)
    report['ok'] = not report['failures'] and report['passed'] == report['expected']
    args.output.write_text(json.dumps(report, indent=2) + '\n')
    print(json.dumps({'ok': report['ok'], 'passed': report['passed'], 'expected': report['expected'], 'report': str(args.output)}))
    return 0 if report['ok'] else 1


if __name__ == '__main__':
    sys.exit(main())
