import { copyFileSync, chmodSync, mkdirSync, writeFileSync, cpSync } from 'node:fs';
import { join } from 'node:path';
const platform = process.env.XFOLD_PLATFORM || `${process.platform}-${process.arch}`;
const binary = platform.startsWith('win32') ? 'xfold.exe' : 'xfold';
const source = process.env.XFOLD_BINARY || join('target', 'release', binary);
const local = join('native', platform); mkdirSync(local, { recursive: true }); copyFileSync(source, join(local, binary)); chmodSync(join(local, binary), 0o755);
const pkg = join('artifacts', 'packages', platform); mkdirSync(join(pkg, 'bin'), { recursive: true }); copyFileSync(source, join(pkg, 'bin', binary)); chmodSync(join(pkg, 'bin', binary), 0o755);
writeFileSync(join(pkg, 'package.json'), JSON.stringify({ name: `@xfold/${platform}`, version: '0.1.0', description: `Xfold native engine for ${platform}`, license: 'MIT', os: [platform.split('-')[0]], cpu: [platform.split('-')[1]], ...(platform.startsWith('linux-') ? { libc: ['glibc'] } : {}), files: ['bin', 'LICENSE', 'THIRD_PARTY_NOTICES.md', 'licenses'] }, null, 2) + '\n');
copyFileSync('LICENSE', join(pkg, 'LICENSE')); copyFileSync('THIRD_PARTY_NOTICES.md', join(pkg, 'THIRD_PARTY_NOTICES.md'));
console.log(`Prepared native/${platform} and ${pkg}`);

cpSync("licenses", join(pkg, "licenses"), { recursive: true });
