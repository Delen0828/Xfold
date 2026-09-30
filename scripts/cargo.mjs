import { spawnSync } from 'node:child_process';
// Normal builds use Cargo from PATH. A custom toolchain can be supplied without
// encoding a developer's workspace or depending on sibling projects.
const cargo = process.env.XFOLD_CARGO || 'cargo';
const result = spawnSync(cargo, process.argv.slice(2), { stdio: 'inherit', env: process.env });
if (result.error) console.error(`Cannot run ${cargo}: ${result.error.message}. Install Rust for source builds.`);
process.exit(result.status ?? 1);
