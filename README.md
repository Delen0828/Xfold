# Xfold

The `light` branch reviews a commit as a small graph of semantic changes.
The graph shows changed modules, their ancestor containers, and direct children.
The right-hand pane is a Git change manager: semantic diff results occupy the
upper half, and selecting a result opens its committed code diff below.

`--ref` (or `--to`) selects the commit to review against its first parent.
A root commit is compared against an empty tree. Use `--from` and `--to` to
review an explicit range. Merge commits default to their first parent.

```sh
xfold /path/to/repository
xfold /path/to/repository --ref HEAD~1
xfold /path/to/repository --from HEAD~1 --to HEAD
xfold build /path/to/repository --ref HEAD --json
```

Normal checkouts and bare repositories work. References must exist locally.
Xfold does not check out commits, execute repository code, modify source,
fetch remotes, or read uncommitted changes. Source is read-only.

## Run this checkout

Node.js 22+, Rust, and a C/C++ build toolchain are needed to build from source.
There is no dependency on the neighboring sem, Normify, or Archify directories.

```sh
npm ci
npm run build
node bin/xfold.mjs /path/to/repository --from HEAD~1 --to HEAD
```

The build compiles the browser assets first and embeds them, including editor
and layout workers, in the native binary. `XFOLD_CARGO` can point to an alternate
Cargo executable; normal Cargo/Rustup environment variables remain supported.

For local distribution:

```sh
npm pack
npm install -g --omit=optional ./xfold-cli-0.1.0.tgz
xfold /path/to/repository
```

The packed local tarball contains the engine for the build machine. It needs
only Node.js to launch; no Rust or separate sem installation is required.
`artifacts/packages/<platform>` contains the matching native npm package.
The release workflow builds macOS arm64/x64, Linux glibc arm64/x64, and Windows
x64 packages. These package names are configured for distribution; this
checkout does not publish anything to npm.

## Explore and compare

- The overview opens directly on the change neighborhood. Double-click a
  container, or use its enter arrow, to narrow the scope.
- The change neighborhood is already unfolded; unrelated siblings remain hidden.
- Use Tab and Enter to select a focused graph node; Alt+Enter enters a container.
- Search with `/`; use `0` to fit the graph, `+`/`−` to zoom, and `M` for the minimap.
- Select a semantic result in the upper review pane or a graph node to open
  the related code change in the lower pane. Cmd/Ctrl-click in the source editor to focus
  the smallest enclosing semantic entity.
- Compare two commits in one graph. Green means added, yellow modified, red
  deleted, blue moved, cyan renamed, and magenta reordered; badges and borders
  duplicate the color meaning.
- Deleted modules open base source. Inline diff is the default; Before/After
  tabs and side-by-side diff are available in the source pane.
- Inspect dependencies, dependents, aggregate edges, or a directed route.
  Comparison traversal always uses the selected base or target snapshot.
- Copy the current URL to reference a commit, module, comparison, and scope.
  The recipient must run Xfold on the same repository and replace the local
  server origin if their port differs.

The neighborhood uses sem's headline changes, excluding location-only shifts
and redundant parent changes. File-only edits remain reviewable when there is
no semantic headline for that file. Parent means containment (file/class/etc.),
not callers or dependencies. Moves retain old and new ancestor context; deleted
entities use base source. Dashed `contains` links show hierarchy.

The graph pane has a 300-node display budget, prioritizing changed modules. Full topology stays on the
backend; drill down or filter when the view reports omitted nodes. Arrows
point from the dependent to its dependency. Cycles are valid.

## Analysis and cache

Light mode reduces graph rendering and layout work. Analysis still builds and
caches full snapshots to preserve sem matching and dependency accuracy; it does
not promise faster initial parsing of a large repository. Semantic results are
loaded in pages of 50, including their module metadata in one request.

All sem entity types and compiled grammars are bundled. Dependency quality
depends on sem's static resolver and language support. Dependencies are
static references, not evidence of runtime execution or certain breakage.
Exact call-site spans are unavailable in the pinned sem API; edge inspection
opens source and target entities instead.

Xfold honors committed `.semrc`, root `.gitattributes`, and `.semignore`, plus
sem's default source exclusions. Tracked Git files are the input, independent
of the user's global Git ignore rules. Unsupported formats remain browsable
as text files but do not receive invented semantic entities. Binary files,
symlinks, submodules, non-UTF-8 files, and files above 4 MiB are recorded with
coverage diagnostics. Syntax recovery is also disclosed.

Snapshots, original source blobs, shared per-file graph chunks, SQLite indexes, and persisted
sem facts are stored under the OS cache directory. Use `--cache-dir PATH` to
override it. The target repository is never used as a cache directory by default.

```sh
xfold /path/to/repository --cache-dir /tmp/xfold-cache --no-open --port 8765
xfold build /path/to/repository --ref HEAD --force --json
xfold build /path/to/repository --extensions .py,.pyi --json
xfold build /path/to/repository --include-excluded --json
```

`--force` uses a cold build and repairs corrupt source objects. A completed
snapshot cache hit does no parsing. Incremental builds reuse sem's file facts
and resolution dependencies. Xfold validates every candidate before atomic
publication. If the pinned sem resolver leaves a dangling reference during
invalidation, Xfold repairs the candidate with a cold build and reports
`incremental-fallback` in the receipt. Failed and cancelled builds preserve
already published snapshots.

## Development and validation

Build a local debug executable from any working directory:

```sh
./scripts/build-local.sh
./target/debug/xfold /path/to/repository
```

The script installs npm dependencies using the lockfile, builds the browser assets, and
embeds them in the Rust executable. If Rust is missing, it installs a minimal
stable toolchain in `.tools/` without changing your shell profile. Node.js 22+
and a C/C++ compiler must already be installed. Downloads require network
access. Existing Cargo installations and `XFOLD_CARGO` are supported.
It uses `npm install` because some npm versions reject `npm ci` when the
optional native distribution packages have not been published yet.
Use `./scripts/build-local.sh --release` for an optimized executable in
`target/release/`. `CARGO_TARGET_DIR` overrides the output directory.
Run the printed executable directly to avoid selecting an older packaged binary.
Rerun the script after changes to refresh the embedded browser assets.

```sh
npm test
npm run test:browser
npm run generate:api
npm run check
node scripts/smoke-package.mjs artifacts/xfold-cli-0.1.0.tgz
```

Browser tests use a clean Chrome session locally, or Playwright Chromium in CI.
They create private Git fixtures, deliberately dirty their working trees, and
verify that the UI still displays committed source. Screenshots and reports
are written to `artifacts/`.

Matplotlib validation requires a local complete Matplotlib Git repository.
For manual testing in the local browser interface, use:

```sh
./scripts/explore-matplotlib.sh             # stackplot comparison
./scripts/explore-matplotlib.sh colormap
./scripts/explore-matplotlib.sh antialiasing
```

This uses the local debug executable and the bare Git repository at
`../sem-test/cache/matplotlib__matplotlib.git`. The browser opens at
`http://127.0.0.1:8765` once analysis completes. Keep the terminal running and
press Ctrl+C to stop. Analyses are cached in `.cache/matplotlib-interactive/`.
Set `XFOLD_MATPLOTLIB_REPO` for another repository or `XFOLD_BINARY` for another
local executable. Pass `--port 8766` to use another port.

To test the local debug build against the three real example commit pairs:

```sh
./scripts/test-matplotlib.sh
./scripts/test-matplotlib.sh --suite all
./scripts/test-matplotlib.sh --release --repo /path/to/matplotlib.git
```

The wrapper defaults to `target/debug/xfold`, honors `CARGO_TARGET_DIR` and
`XFOLD_BINARY`, and accepts `--binary`, `--cases`, and `--output` overrides.
It works from any working directory and writes a timestamped JSON report and
stderr log in `artifacts/`. Build first with `scripts/build-local.sh`; use
`scripts/build-local.sh --release` before testing an optimized build.
These checks compare Xfold's incremental and cold graphs, not Matplotlib's
runtime tests. Debug builds can take several minutes.

The default paths reuse this workspace's existing sem-test cache and cases;
pass explicit locations elsewhere:

```sh
python3 scripts/benchmark.py --repo /path/to/matplotlib.git --suite smoke
python3 scripts/benchmark.py --repo /path/to/matplotlib.git --suite history
python3 scripts/benchmark.py --repo /path/to/matplotlib.git \
  --cases /path/to/local-source-all --suite all
```

The full suite checks 3 real commit pairs, 14 first-parent transitions, and
34 explicitly synthetic SWE-Bench commits. Each target's incremental graph
must equal a separate cold build. It does not run Matplotlib or its tests.
Reports record timings, file/edge reuse, object size, hardware context, and
sampled worker memory. Temporary repository objects and caches are removed.

See [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md) for code ownership and cache/identity details,
[docs/VALIDATION.md](docs/VALIDATION.md) for measured results, and [SPEC.md](SPEC.md) for the contract and [docs/openapi.json](docs/openapi.json)
for the generated API. `/api/v1/openapi.json` exposes the same schema at runtime.
The API only accepts its loopback origin and the repository selected at launch.
