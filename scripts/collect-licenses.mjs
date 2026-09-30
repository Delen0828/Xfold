import { spawnSync } from 'node:child_process';
import { copyFileSync, mkdirSync, readdirSync, readFileSync, writeFileSync, existsSync } from 'node:fs';
import { join, dirname } from 'node:path';
const out = 'licenses'; mkdirSync(out, { recursive: true });
const records = [];
function collect(root, destination, depth = 0) {
  if (!existsSync(root) || depth > 3) return;
  for (const entry of readdirSync(root, { withFileTypes: true })) {
    const path = join(root, entry.name);
    if (entry.isFile() && /^(licen[sc]e|copying|copyright|notice|ofl|third[-_]?party.*notices?)([._-]|$)/i.test(entry.name)) {
      mkdirSync(destination, { recursive: true }); copyFileSync(path, join(destination, entry.name));
    } else if (entry.isDirectory() && ['libgit2', 'libssh2', 'libz-sys', 'sqlite3', 'vendor', 'assets', 'fonts'].includes(entry.name)) collect(path, join(destination, entry.name), depth + 1);
  }
}
const cargo = spawnSync(process.env.XFOLD_CARGO || 'cargo', ['metadata', '--locked', '--offline', '--format-version=1'], { encoding: 'utf8', maxBuffer: 30 * 1024 * 1024 });
if (cargo.status !== 0) throw new Error(cargo.stderr || cargo.error?.message);
for (const pkg of JSON.parse(cargo.stdout).packages) {
  collect(dirname(pkg.manifest_path), join(out, 'rust', `${pkg.name}-${pkg.version}`));
  records.push({ ecosystem: 'rust', name: pkg.name, version: pkg.version, license: pkg.license, repository: pkg.repository });
}
const lock = JSON.parse(readFileSync('package-lock.json', 'utf8'));
for (const [path, entry] of Object.entries(lock.packages)) {
  if (!path || !existsSync(join(path, 'package.json'))) continue;
  const pkg = JSON.parse(readFileSync(join(path, 'package.json'), 'utf8'));
  collect(path, join(out, 'npm', `${pkg.name.replaceAll('/', '__')}-${pkg.version}`));
  records.push({ ecosystem: 'npm', name: pkg.name, version: pkg.version, license: pkg.license, repository: pkg.repository });
}
copyFileSync('vendor/LICENSE-MIT', join(out, 'sem-LICENSE-MIT'));
copyFileSync('vendor/LICENSE-APACHE', join(out, 'sem-LICENSE-APACHE'));
writeFileSync(join(out, 'manifest.json'), JSON.stringify(records, null, 2) + '\n');
console.log(`Collected notices for ${records.length} locked packages.`);
