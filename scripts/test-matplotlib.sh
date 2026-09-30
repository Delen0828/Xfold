#!/usr/bin/env bash
# Compare incremental and cold Xfold graphs for the Matplotlib fixtures.
set -euo pipefail

project_dir="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)"
profile=debug
suite=smoke
repo="$project_dir/../sem-test/cache/matplotlib__matplotlib.git"
cases="$project_dir/../sem-test/results/local-source-all"
binary="${XFOLD_BINARY:-}"
output=''

usage() {
  cat <<'EOF'
Usage: test-matplotlib.sh [options]

Uses target/debug/xfold and the workspace's cached Matplotlib Git repository.
By default, checks the stackplot, colormap, and antialiasing commit pairs.
This analyzes committed code; it does not execute Matplotlib's own tests.

  --release           Use target/release/xfold instead
  --suite SUITE       smoke (default), history, synthetic, or all
  --repo PATH         Local complete Matplotlib Git repository (bare or checkout)
  --cases PATH        SWE-Bench case directory, needed for synthetic/all
  --binary PATH       Explicit executable (also accepts XFOLD_BINARY)
  --output PATH       JSON report (default: a timestamped file in artifacts/)
  -h, --help          Show this help

Relative argument paths are resolved from your current working directory.
CARGO_TARGET_DIR is resolved from Xfold, matching scripts/build-local.sh.
Build first with scripts/build-local.sh [--release]. Debug runs are slower.
EOF
}

while (( $# )); do
  case "$1" in
    -h|--help) usage; exit 0 ;;
    --release) profile=release; shift ;;
    --suite|--repo|--cases|--binary|--output)
      if (( $# < 2 )) || [[ -z "$2" || "$2" == --* ]]; then
        echo "Missing value for $1" >&2; exit 2
      fi
      case "$1" in
        --suite) suite="$2" ;;
        --repo) repo="$2" ;;
        --cases) cases="$2" ;;
        --binary) binary="$2" ;;
        --output) output="$2" ;;
      esac
      shift 2 ;;
    *) echo "Unknown argument: $1 (use --help)" >&2; exit 2 ;;
  esac
done
case "$suite" in
  smoke|history|synthetic|all) ;;
  *) echo "Unknown suite: $suite" >&2; exit 2 ;;
esac
for tool in python3 git; do
  command -v "$tool" >/dev/null 2>&1 || { echo "Missing required tool: $tool" >&2; exit 1; }
done
if [[ -z "$binary" ]]; then
  target_dir="${CARGO_TARGET_DIR:-target}"
  [[ "$target_dir" == /* ]] || target_dir="$project_dir/$target_dir"
  binary="$target_dir/$profile/xfold"
fi
if [[ ! -x "$binary" ]]; then
  echo "Xfold executable not found: $binary. Run scripts/build-local.sh (or --release) first." >&2
  exit 1
fi
# An absolute path prevents subprocess from treating a relative filename as a PATH lookup.
binary="$(cd -- "$(dirname -- "$binary")" && pwd)/$(basename -- "$binary")"
if ! git -C "$repo" rev-parse --git-dir >/dev/null 2>&1; then
  echo "Matplotlib Git repository not found: $repo. Supply --repo PATH." >&2
  exit 1
fi
if [[ "$suite" == synthetic || "$suite" == all ]] && [[ ! -d "$cases" ]]; then
  echo "SWE-Bench cases not found: $cases. Supply --cases PATH." >&2
  exit 1
fi
output="${output:-$project_dir/artifacts/matplotlib-local-$suite-$(date +%Y%m%d-%H%M%S)-$$.json}"
printf 'Executable: %s\nSuite: %s\nReport: %s\n' "$binary" "$suite" "$output"
exec python3 "$project_dir/scripts/benchmark.py" \
  --binary "$binary" --repo "$repo" --cases "$cases" \
  --suite "$suite" --output "$output"
