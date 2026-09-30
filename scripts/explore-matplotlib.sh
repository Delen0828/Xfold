#!/usr/bin/env bash
# Open a Matplotlib fixture comparison in the local Xfold browser UI.
set -euo pipefail
project_dir="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)"

if [[ "${1:-}" == --help || "${1:-}" == -h ]]; then
  cat <<'EOF'
Usage: explore-matplotlib.sh [stackplot|colormap|antialiasing] [Xfold options]

Opens the selected comparison (default: stackplot) in your browser after analysis.
Uses target/debug/xfold and the workspace's cached Matplotlib Git repository.
The server stays in the foreground; press Ctrl+C to stop it.

Examples:
  ./scripts/explore-matplotlib.sh
  ./scripts/explore-matplotlib.sh colormap --port 8766
  XFOLD_BINARY="$PWD/target/release/xfold" ./scripts/explore-matplotlib.sh

Environment: XFOLD_BINARY, XFOLD_MATPLOTLIB_REPO, CARGO_TARGET_DIR.
Additional Xfold options include --port, --cache-dir, and --no-open.
EOF
  exit 0
fi
example=stackplot
if (( $# )) && [[ "$1" != -* ]]; then example="$1"; shift; fi
for tool in python3 git; do
  command -v "$tool" >/dev/null 2>&1 || { echo "Missing required tool: $tool" >&2; exit 1; }
done
target_dir="${CARGO_TARGET_DIR:-target}"
[[ "$target_dir" == /* ]] || target_dir="$project_dir/$target_dir"
binary="${XFOLD_BINARY:-$target_dir/debug/xfold}"
repo="${XFOLD_MATPLOTLIB_REPO:-$project_dir/../sem-test/cache/matplotlib__matplotlib.git}"
if [[ ! -x "$binary" ]]; then
  echo "Xfold executable not found: $binary. Run scripts/build-local.sh first." >&2
  exit 1
fi
if ! git -C "$repo" rev-parse --git-dir >/dev/null 2>&1; then
  echo "Matplotlib repository not found: $repo. Set XFOLD_MATPLOTLIB_REPO." >&2
  exit 1
fi
pair="$(python3 - "$project_dir/fixtures/matplotlib.json" "$example" <<'PY'
import json
import sys
with open(sys.argv[1]) as source:
    pairs = json.load(source)['pairs']
pair = next((p for p in pairs if p['name'] == sys.argv[2]), None)
if pair is None:
    sys.exit('Unknown example. Choose: ' + ', '.join(p['name'] for p in pairs))
print(pair['base'], pair['target'])
PY
)"
read -r base target <<< "$pair"
printf 'Opening %s comparison using %s\nInitial analysis may take several minutes. The browser opens when ready.\n' "$example" "$binary"
exec "$binary" "$repo" --from "$base" --to "$target" \
  --cache-dir "$project_dir/.cache/matplotlib-interactive" --port 8765 "$@"
