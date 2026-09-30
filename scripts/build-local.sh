#!/usr/bin/env bash
# Build the embedded web UI and native engine for this checkout.
set -euo pipefail

project_dir="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$project_dir"

profile=debug
case "${1:-}" in
  '') ;;
  --release) profile=release ;;
  -h|--help)
    echo "Usage: $0 [--release]"
    echo "Installs npm dependencies and missing Rust, then builds Xfold (debug by default)."
    exit 0 ;;
  *) echo "Unknown argument: $1" >&2; exit 2 ;;
esac
if (( $# > 1 )); then
  echo "Expected at most one argument; use --help." >&2
  exit 2
fi

for tool in node npm cc c++; do
  if ! command -v "$tool" >/dev/null 2>&1; then
    echo "Missing $tool. Install Node.js 22+ and a C/C++ build toolchain first." >&2
    exit 1
  fi
done
node -e 'if (Number(process.versions.node.split(".")[0]) < 22) { console.error("Node.js 22+ is required."); process.exit(1); }'

# Prefer an explicitly selected Cargo or an existing toolchain. Keep any
# bootstrapped toolchain inside the checkout without changing shell profiles.
if [[ -z "${XFOLD_CARGO:-}" ]]; then
  if command -v cargo >/dev/null 2>&1; then
    export XFOLD_CARGO="$(command -v cargo)"
  elif [[ -x "${CARGO_HOME:-$HOME/.cargo}/bin/cargo" ]]; then
    export PATH="${CARGO_HOME:-$HOME/.cargo}/bin:$PATH"
    export XFOLD_CARGO="$(command -v cargo)"
  else
    export CARGO_HOME="$project_dir/.tools/cargo"
    export RUSTUP_HOME="$project_dir/.tools/rustup"
    export PATH="$CARGO_HOME/bin:$PATH"
    if [[ ! -x "$CARGO_HOME/bin/cargo" ]]; then
      command -v curl >/dev/null 2>&1 || { echo "curl is required to install Rust." >&2; exit 1; }
      mkdir -p "$project_dir/.tools"
      installer="$(mktemp "$project_dir/.tools/rustup-init.XXXXXX")"
      trap 'rm -f "$installer"' EXIT
      curl --proto '=https' --tlsv1.2 --fail --show-error --location https://sh.rustup.rs -o "$installer"
      sh "$installer" -y --profile minimal --default-toolchain stable --no-modify-path
      rm -f "$installer"
      trap - EXIT
    fi
    export XFOLD_CARGO="$CARGO_HOME/bin/cargo"
  fi
fi
"$XFOLD_CARGO" --version

# Install devDependencies even when the caller has NODE_ENV=production.
# Keep npm's cache local as well; do not require global package installation.
# npm ci rejects the unpublished optional @xfold/* distribution packages on
# some npm versions. npm install tolerates their absence for source builds.
npm install --include=dev --no-audit --no-fund --cache "$project_dir/.npm-cache"
npm run build:web
cargo_args=(build --locked --bin xfold)
if [[ "$profile" == release ]]; then cargo_args+=(--release); fi
node scripts/cargo.mjs "${cargo_args[@]}"

target_dir="${CARGO_TARGET_DIR:-$project_dir/target}"
binary="$target_dir/$profile/xfold"
"$binary" --help >/dev/null
printf '\nBuild complete. Run:\n  %q /path/to/repository\n' "$binary"
