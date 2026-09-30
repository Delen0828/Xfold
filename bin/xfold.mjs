#!/usr/bin/env node
import { existsSync } from 'node:fs';
import { createRequire } from 'node:module';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { spawn } from 'node:child_process';
const root = dirname(dirname(fileURLToPath(import.meta.url)));
const platform = `${process.platform}-${process.arch}`;
const name = `xfold${process.platform === 'win32' ? '.exe' : ''}`;
const require = createRequire(import.meta.url);
let executable = process.env.XFOLD_BINARY;
if (!executable) {
  try { executable = require.resolve(`@xfold/${platform}/bin/${name}`); } catch { /* packed standalone distribution or source build */ }
}
if (!executable) executable = [join(root, 'native', platform, name), join(root, 'target', 'release', name), join(root, 'target', 'debug', name)].find(existsSync);
if (!executable) {
  console.error(`No Xfold engine for ${platform}. Install the matching @xfold/${platform} package, or build this source checkout with npm run build.`);
  process.exit(1);
}
const child = spawn(executable, process.argv.slice(2), { stdio: 'inherit', windowsHide: true });
child.on('error', error => { console.error(`Cannot launch Xfold: ${error.message}`); process.exitCode = 1; });
for (const signal of ['SIGINT', 'SIGTERM']) process.on(signal, () => child.kill(signal));
child.on('exit', (code, signal) => { process.exitCode = code ?? (signal ? 1 : 0); });
