# Third-party notices

Xfold embeds sem-core from Ataraxy Labs, revision
`e830de88ee2267c3d93342cc367d00734d376a12`, version 0.25.0.
Its source is in `vendor/sem-core`; its MIT and Apache-2.0 licenses are in
`vendor/LICENSE-MIT` and `vendor/LICENSE-APACHE`. Distribution license texts
are included in `licenses/`.

Two small packaging/API patches are applied to that source:

1. `suppress_redundant_parents` is public so Xfold can reuse sem's unchanged
   suppression algorithm with its conservative entity correspondences.
2. sem-core's optional git2 dependency disables default network features.
   Xfold uses local libgit2 object reads, with vendored libgit2 and no SSH or
   OpenSSL installation prerequisite.

The Rust engine includes tree-sitter and its language grammars, libgit2,
SQLite, Axum, Tokio, and the dependencies pinned in Cargo.lock. libgit2 is
GPL-2.0 with a linking exception. SQLite is public domain. Other dependency
licenses and original copyright notices are collected in `licenses/` by the
release packaging script. That collection includes bundled C library notices.

The browser bundles React, React DOM, React Flow, Monaco Editor, ELK.js, and
their dependencies from package-lock.json. React, React Flow, and Monaco
are MIT licensed. ELK.js is EPL-2.0 licensed; the exact unmodified package
can be obtained from https://github.com/kieler/elkjs and its version is
recorded in package-lock.json. Bundled Monaco codicon font notices are also
retained. No third-party assets are fetched at runtime.

Xfold's normalization and receipt design is inspired by dsh-normify,
revision `ed404e5b8db0010ff57ec8486ec8749421072545` (MIT).
Its interaction design is inspired by Archify, revision
`d5a1333d7447c866a765adac7d4d062f2f02e4d2` (MIT).
Neither project is a runtime dependency, and their renderers are not copied.
