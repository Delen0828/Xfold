# Xfold v1 specification

## Product contract

Xfold is a local code exploration tool that turns a Git repository into a
browsable graph of semantic modules. Users navigate from repository structure
to individual entities, inspect exact source, follow dependencies, and compare
commits.

- A semantic module is an entity from sem's language/format-specific parser.
  Preserve its original entity type, name, identity, containment, and source.
- Repository, directory, and file nodes provide navigation and aggregation;
  distinguish them from extracted semantic entities.
- Deliver an npm launcher with a native backend and bundled browser assets.
- Support committed local revisions, including bare repositories. Default to
  HEAD. Resolve references to immutable full commit IDs before analysis.
- Use one comparison graph, with sem's colors and additional shape/badge cues.
- Keep the source pane read-only. Default to a resizable 2:1 graph/source split.
- Exclude live working-tree analysis, source editing, remote cloning, AI grouping,
  and runtime code execution from v1.

Commands:

```sh
xfold /path/to/repository
xfold /path/to/repository --ref <revision>
xfold /path/to/repository --from <base> --to <target>
xfold build /path/to/repository --ref <revision> --json
```

Start a loopback server, print its URL, and open the browser unless --no-open
is supplied. Compare selected snapshots directly. A commit-to-parent comparison
defaults to its first parent. All runtime assets and analysis operate offline.

## Architecture and data

Rust owns Git object reading (libgit2), sem-core analysis, canonical storage,
SQLite indexes, asynchronous build jobs, and the Axum API. A separate, persistent
worker owns GraphSession and private materialized files. The coordinator owns
publication. A failed or cancelled worker cannot replace a completed snapshot.

React and TypeScript own interaction, React Flow owns graph presentation,
ELK.js runs layout in a worker, and Monaco displays committed source and diffs.
Layout, camera, and selection are not semantic facts.

Reference revisions:

- sem: e830de88ee2267c3d93342cc367d00734d376a12. Embed and vendor sem-core.
- Normify: ed404e5b8db0010ff57ec8486ec8749421072545. Adopt normalized parent-only
  containment, derived aggregates, fingerprints, deterministic artifacts, and
  validation receipts. Do not create authored Markdown modules automatically.
- Archify: d5a1333d7447c866a765adac7d4d062f2f02e4d2. Adapt focus, camera,
  navigation, minimap, search, finite motion, and reduced-motion behavior.

Canonical records:

| Record | Meaning |
|---|---|
| Snapshot | Repository, commit/tree IDs, profile/configuration, engine version, hashes, coverage |
| Module | Snapshot-scoped node ID, sem ID, type, name, parent, source, content/structure hashes |
| Container | Repository/directory/file navigation node with derived child aggregates |
| SourceRef | Relative path, stored blob, inclusive lines, optional half-open byte span |
| Dependency | Dependent → dependency, type, extractor provenance |
| Comparison | Base/target, one-to-one correspondence, node/file/edge deltas, summaries |
| BuildReceipt | Input/output hashes, validation, timing, parsing and resolution reuse |

Store containment once through parent; derive children, ancestors, and counts.
Store each semantic edge once; derive adjacency and visible aggregate edges.
Dependency cycles are legal. Classes can contain methods and also be modules.
File-level changes outside entities remain visible.

Identity is independent of content. Equal function bodies remain separate
entities. Deterministic snapshot node IDs derive from sem identity; comparison
correspondence handles renames/moves without inventing global history identity.

Preserve source bytes and line endings, and explicitly convert byte offsets to
editor coordinates. Every node and source response belongs to one immutable
snapshot. Every aggregate edge expands to its original semantic relationships.

Use SHA-256 content-addressed artifacts plus SQLite WAL indexes for snapshots,
containment, adjacency, search, and comparisons. Cache outside the repository,
with --cache-dir override. Keep source, graph, topology, and layout hashes
separate. Canonical graph hashes exclude timing, machine paths, and build order.
Hash sorted edge records; do not recursively hash cyclic dependency neighbors.

## Incremental build contract

1. Resolve the requested revision and analysis profile.
2. Return a verified completed snapshot when available, with zero parsing.
3. Enumerate Git blobs and apply committed parser configuration and source scope.
4. Materialize source privately; never check out or modify user source.
5. Update changed files and pass both changed paths and the complete new file list
   to sem's GraphSession. Warm-start from compatible persisted facts after restart.
6. Normalize modules, relationships, source references, and derived indexes.
7. Validate the candidate and write immutable artifacts and a receipt.
8. Atomically publish the snapshot and indexes, then announce readiness.

Parser/grammar/resolver versions, extraction scope, and committed configuration
are cache inputs. Preserve sem's recorded lookup dependencies, including misses:
unchanged files can require resolution after symbols are added or removed.
Conservative cold rebuilding is permitted; serving stale edges is not.

**Invariant:** every incremental route to a commit and configuration produces the
same canonical graph as a cold build. Validate containment, endpoint existence,
source ranges, and object integrity. Recovery must be visible in the receipt.
Coverage exclusions and parser recovery must never look like complete analysis.

Jobs progress queued → running → validating → ready, or end failed/cancelled.
Requests are deduplicated. Only completed snapshots are queryable. Ordered SSE
events and job status allow reconnection; stale UI requests cannot replace newer
selections. Traversals always choose exactly one snapshot.

## Backend interface

All routes below are under /api/v1. DTOs generate OpenAPI schemas and TypeScript
types. Stable ordering, pagination, and explicit truncation apply where relevant.

| Operation | Endpoint |
|---|---|
| Workspace defaults and capabilities | GET /workspace |
| Local refs and paginated commit history | GET /refs; GET /commits |
| Build/open a snapshot | POST /snapshots |
| Snapshot summary | GET /snapshots/{id} |
| Job state, progress, cancellation | GET /jobs/{id}; GET /jobs/{id}/events; POST /jobs/{id}/cancel |
| Children and aggregates | GET /snapshots/{id}/children |
| Module metadata and ancestry | GET /snapshots/{id}/modules/{moduleId} |
| Search by name/path/type/language | GET /snapshots/{id}/search |
| Committed source and reverse source lookup | GET /snapshots/{id}/source; GET /snapshots/{id}/modules-at |
| Graph projection | POST /graph-views |
| Aggregate relationship expansion | GET /graph-views/{id}/edges/{edgeId} |
| Dependencies/dependents | POST /traversals |
| Directed shortest path | POST /paths |
| Build a comparison | POST /comparisons |
| Comparison and paginated changes | GET /comparisons/{id}; GET /comparisons/{id}/changes |
| Before/after source and hunks | GET /comparisons/{id}/source-diff |
| Coverage and receipt | GET /snapshots/{id}/diagnostics; GET /snapshots/{id}/receipt |

Graph requests accept exactly one snapshot or comparison, hierarchy scope,
expanded nodes, entity/change/edge filters, and a display budget (default 300).
Full topology stays on the backend. Out-of-scope relationships use boundary nodes.

sem provides edge endpoints/types but no exact call-site spans. V1 reports this
precision limitation and opens endpoint source; it never invents call locations,
confidence scores, or runtime impact. Path queries in comparison mode must use
base or target, never the union of relationships across time.

Bind to loopback, enforce same-origin requests, serve source only through snapshot
manifests, escape displayed data, and never execute repository code or Git hooks.

## Interaction and comparison

Start with a directory/file overview. Support breadcrumbs, enter/expand/collapse,
search, type/change/relationship filters, pan/zoom, fit/reset, minimap, keyboard
navigation, and links containing commits, scope, and entity identity.

Selecting an entity opens the full committed file and highlights its exact range.
Selecting a file opens the file; directories show children. Source-position
selection focuses the smallest enclosing entity. Edge selection exposes original
relationships. Source loading must not wait for animation completion.

Compare complete entity snapshots. Require one-to-one matching, using sem matching
and conservative uniqueness checks. Ambiguity remains added/deleted with a
diagnostic. Track addition/deletion, structural/cosmetic edits, rename/move/reorder,
dependency changes, location shifts, and changed descendants independently.
Preserve sem's parent suppression for headline counts and report descendant
counts separately.

The comparison union renders matched entities once, additions with target source,
deletions with base source, and moves at the target location with former-location
indicators. Aggregate edges retain added/removed/unchanged counts independently.

| Status | Color | Cue |
|---|---|---|
| Added | Green | ⊕, emphasized border |
| Modified | Yellow | ∆ |
| Deleted | Red | ⊖, dashed outline |
| Moved | Blue | → |
| Renamed | Cyan | ↻ |
| Reordered | Magenta | ↕ |
| Cosmetic | Muted neutral | ~ |

Use a distinct focus ring. Default source comparisons to inline diff, with
Before/After tabs and optional side-by-side viewing. Explicitly indicate missing
sides. ELK layout runs in a worker and is cached independently of source text.
Finite focus/layout transitions respect reduced motion and document visibility;
superseded transitions are cancelled. Never continuously animate dependencies.

## Acceptance and delivery

Pin fixtures in fixtures/matplotlib.json: stackplot, colormap handling, and text
antialiasing real commit pairs; fourteen first-parent transitions; and all 34
local Matplotlib SWE-Bench cases as explicitly synthetic child commits.
Never execute Matplotlib or its test suite. Validate extraction, topology,
comparison, source correspondence, and full/incremental equality.

Small repositories must exercise restart, A→B→A, skipped commits, branches,
symbol/import/config invalidation, duplicate bodies, rename/move/reorder,
cosmetic/line shifts, cycles, aggregate expansion, Unicode/CRLF, deleted source,
unsupported/malformed input, missing objects, corruption, cancellation, and
worker recovery. Browser checks cover interaction, links, read-only source,
comparison, keyboard controls, reduced motion, and offline assets.

Record hardware, timings, peak memory, parsing/resolution reuse, artifact size,
and UI behavior. Require zero parsing for completed cache hits and demonstrable
incremental reuse. Measurements must be actual results, not inferred guarantees.

Build in order: snapshot engine and correctness; query/UI exploration;
comparison and source diff; validation and platform packaging.
Ship Node.js 22+ launchers and native packages for macOS arm64/x64, Linux glibc
arm64/x64, and Windows x64. Bundle web assets and notices, lock dependencies,
test packed installation and offline operation, and do not require end users to
install Rust or sem. Platform CI results, not configuration alone, establish
cross-platform release support.
