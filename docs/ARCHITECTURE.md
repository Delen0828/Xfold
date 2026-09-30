# Implementation notes

Xfold's public contract is [SPEC.md](../SPEC.md). The implementation keeps
analysis, immutable facts, comparison, and browser state separate.

| Code | Ownership |
| --- | --- |
| `src/repo.rs` | Local Git object access, revision resolution, refs/history, portable path validation. |
| `src/engine.rs` | Private materialization, parser configuration, resident `GraphSession`, persisted facts, source/graph validation. |
| `src/store.rs` | SHA-256 objects, per-file graph chunks, atomic SQLite publication, indexed search. |
| `src/compare.rs` | One-to-one correspondence, ambiguous matches, independent change flags, sem parent suppression, edge deltas. |
| `src/query.rs` | Immutable hierarchy indexes, bounded projections, edge aggregation, source/diffs, directed traversals/routes. |
| `src/jobs.rs` | Serialized jobs, worker subprocess lifetime, progress sequences, cancellation and publication. |
| `src/api.rs` | Loopback API, source access through manifests, cached immutable queries, embedded browser assets, OpenAPI. |
| `web/src/App.tsx` | Revision selection, graph navigation, camera/layout caches, keyboard/motion and request ordering. |
| `web/src/SemanticChanges.tsx` | Paginated semantic results and selection in the upper half of the review pane. |
| `web/src/CodePane.tsx` | Read-only Monaco source/diff models, byte-coordinate conversion, model lifetime. |
| `web/src/layout.worker.ts` | Local ELK worker, deterministic bounded layout. |

## Build and publication

The launcher chooses a platform executable. That executable owns the HTTP
server and one long-lived parser worker. A newline JSON protocol passes build
requests and progress between them. Large candidate graphs pass as object-store
hashes, not as pipe payloads. Parser cancellation kills the child; a subsequent
job starts another child and restores compatible facts from disk.

The worker never publishes SQLite snapshot rows. It produces and validates a
candidate; the coordinator checks cancellation and publishes all indexes and
the manifest in one transaction. Failed or interrupted builds can leave
unreferenced immutable objects, but cannot publish partial snapshots.

A cache hit verifies committed source and graph objects, returns without
parsing, and does not rewrite the snapshot's indexes. Its CLI receipt reports
zero analysis; the stored receipt retains the original build measurements.

The engine reads only committed blobs. Materialization is private to the cache,
with an exclusive worker lock. `.semrc`, root `.gitattributes`, `.semignore`,
extraction scope, sem revision, and the locked dependency fingerprint determine
cache compatibility. A dependency-lock change invalidates both snapshot lookup
and persisted facts. Normalized output hashes are separate: identical semantic
output may have the same graph hash under two compatible implementations.

The pinned sem incremental resolver has a known deletion case that retains a
reference to an absent entity. Xfold checks endpoint integrity and repairs such
candidates with a cold build. Receipts identify `incremental-fallback`; this is
covered by the symbol-deletion regression test. Other invalidation decisions
remain sem's, including its conservative resolution fallback.

## Objects and indexes

A snapshot manifest holds metadata and hashes of file-sized module/dependency
chunks. Module chunks are sorted by semantic ID; dependency chunks are sorted
and grouped by their source file. Unchanged chunks share storage across commits.
Original UTF-8 source bytes, including CRLF, are separate content-addressed
objects. Parent links represent containment; edges represent dependencies.

SQLite WAL stores completed-snapshot lookup, parent/source indexes, directed
adjacency, search, receipts, and comparisons. Immutable snapshots and comparison
objects are cached in the server. Hierarchy counts and ancestor maps are derived
once per artifact and reused across projections; complete projections have a
bounded cache as well. These caches do not affect canonical output.

Snapshot identity includes repository, full commit, extraction profile, engine
and locked dependency fingerprint. Entity IDs derive from sem identity, never
from function body hashes. An entity reference is the pair `(snapshot, module)`;
`SourceRef` is interpreted inside that immutable snapshot envelope. A source-only
edit changes source records and anchors while preserving topology/layout inputs.

## Comparisons and displayed identities

Ordinary sem identities establish correspondence first. Overloads that reuse
line-based sem IDs with different bodies are reconsidered using sem similarity.
Rename/move candidates require a reciprocal, unique best match. Ambiguous
candidates remain added/deleted, with a diagnostic.

Comparison view IDs are distinct from base entity IDs. Matched and added nodes
use target IDs; deleted nodes use `b-<base-id>`. `before` always supplies the real
base entity ID. This prevents an ambiguous deleted entity from colliding with an
added entity that reused the same sem ID. Parent projections and aggregate edges
use the same correspondence, while edge inspection returns original semantic
endpoints and their snapshot/source anchors.

A single visual node can have multiple change flags. Added/removed edges remain
separate counts even when they collapse to one visible relationship. Traversal
and route requests require a snapshot ID; comparison union edges are never fed
to reachability queries.

## Browser and API

Graph responses are bounded, disclose omissions, and keep every aggregate's
underlying relationships on the server. Source responses contain full committed
files and selected anchors. Source position lookup uses UTF-8 byte offsets;
Monaco's UTF-16 columns are converted using the exact source text.

The browser uses a 2:1 split, custom React Flow nodes, finite transitions and
local workers/assets. Diff selection uses base source for deleted nodes. An
expanded side-by-side view temporarily widens the source pane. Camera and
layout state are browser-local. Request sequences and aborts prevent stale
responses from replacing a newer selection.

`docs/openapi.json`, `web/src/schema.ts`, and `web/src/generated-client.ts` are
generated from backend DTOs and endpoint metadata. EventSource transports job
SSE; ordinary operations use the generated client or typed query helpers.

## Release boundaries

No repository code, hooks, checkout filters, or runtime analyses are executed.
The server binds only to loopback and checks Host/Origin. Unsupported files and
syntax recovery are visible coverage diagnostics. Static dependency precision
is limited to sem's extracted entity endpoints; exact call sites are unavailable.

The local verified artifact is macOS arm64. The workflow defines the other
requested native targets; successful CI builds and packed smoke tests on those
hosts are still needed before claiming those platforms as validated releases.

## Light projection

`GraphRequest.light` selects the change neighborhood for comparisons. The browser
always uses it; snapshot/build and full graph APIs remain available to existing
tools. The projection index caches sem headline seeds, their ancestors (including
old ancestors of moved modules), and direct children. File-only changes are
fallback seeds. Boundary dependencies cannot introduce unrelated modules. The
300-node budget prefers seeds; narrowing scope and semantic result selection
provide access to changes outside that budget. Client-side containment links
participate in layout but are not represented as semantic dependency edges.

`GET /comparisons/{id}/changes` returns paginated seeds with inline `module`
metadata, keeping review independent of graph scope and display truncation.
An omitted/empty comparison base resolves to the target's first parent. Root
commits use the reserved `:empty` snapshot revision, materialized entirely in
the private cache without writing objects to the user's Git repository.
