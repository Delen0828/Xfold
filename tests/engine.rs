use git2::{Repository, Signature};
use std::{collections::BTreeSet, fs, path::Path};
use tempfile::TempDir;
use xfold::{
    compare,
    engine::Engine,
    model::*,
    query,
    store::{file_id, module_id, Store},
};

fn commit(repo: &Repository, files: &[(&str, Option<&str>)]) -> String {
    let root = repo.workdir().unwrap();
    let mut index = repo.index().unwrap();
    for (path, content) in files {
        let full = root.join(path);
        if let Some(content) = content {
            fs::create_dir_all(full.parent().unwrap()).unwrap();
            fs::write(&full, content).unwrap();
            index.add_path(Path::new(path)).unwrap();
        } else {
            fs::remove_file(full).unwrap();
            index.remove_path(Path::new(path)).unwrap();
        }
    }
    index.write().unwrap();
    let tree = repo.find_tree(index.write_tree().unwrap()).unwrap();
    let sig = Signature::now("Xfold test", "test@localhost").unwrap();
    let parent = repo.head().ok().and_then(|h| h.peel_to_commit().ok());
    repo.commit(
        Some("HEAD"),
        &sig,
        &sig,
        "fixture",
        &tree,
        &parent.iter().collect::<Vec<_>>(),
    )
    .unwrap()
    .to_string()
}
fn setup() -> (TempDir, Repository, Store) {
    let dir = TempDir::new().unwrap();
    let repo = Repository::init(dir.path().join("repo")).unwrap();
    let store = Store::open(dir.path().join("cache")).unwrap();
    (dir, repo, store)
}
fn build(e: &mut Engine, revision: &str, force: bool) -> (Snapshot, BuildReceipt) {
    e.build(
        &BuildRequest {
            revision: revision.into(),
            profile: Profile::default(),
            force,
        },
        |_, _| {},
    )
    .unwrap()
}
fn module<'a>(s: &'a Snapshot, name: &str) -> &'a Module {
    s.modules
        .values()
        .find(|m| m.is_entity() && m.name == name)
        .unwrap_or_else(|| panic!("missing {name}"))
}

#[test]
fn incremental_matches_cold_and_restores_a() {
    let (_dir, repo, store) = setup();
    let a = commit(
        &repo,
        &[
            ("lib.py", Some("def target():\n    return 1\n")),
            (
                "main.py",
                Some("from lib import target\ndef run():\n    return target()\n"),
            ),
        ],
    );
    let mut e = Engine::new(repo.workdir().unwrap(), store.clone()).unwrap();
    let (sa, _) = build(&mut e, &a, false);
    store.publish(&sa, &build_receipt(&sa)).unwrap();
    let b = commit(
        &repo,
        &[(
            "lib.py",
            Some("def target():\n    return 2\n\ndef added():\n    return target()\n"),
        )],
    );
    let (sb, receipt) = build(&mut e, &b, false);
    assert_eq!(receipt.stats.mode, "incremental");
    assert_eq!(receipt.stats.files_changed, 1);
    assert!(receipt.stats.edges_reused > 0 || receipt.stats.files_reused > 0);
    let (cold, _) = build(&mut e, &b, true);
    assert_eq!(sb.graph_hash, cold.graph_hash);
    assert_eq!(sb.edges, cold.edges);
    let (back, _) = build(&mut e, &a, true);
    assert_eq!(back.graph_hash, sa.graph_hash);
    store.publish(&sb, &receipt).unwrap();
    drop(e);
    let mut resumed = Engine::new(repo.workdir().unwrap(), store.clone()).unwrap();
    let (_, hit) = build(&mut resumed, &b, false);
    assert_eq!(hit.stats.mode, "cache-hit");
    assert_eq!(hit.stats.files_parsed, 0);
    let c = commit(
        &repo,
        &[(
            "lib.py",
            Some("def target():\n    return 3\n\ndef added():\n    return target()\n"),
        )],
    );
    let (warm, r) = build(&mut resumed, &c, false);
    assert_eq!(r.stats.mode, "warm-start");
    let (cold, _) = build(&mut resumed, &c, true);
    assert_eq!(warm.graph_hash, cold.graph_hash);
}
fn build_receipt(s: &Snapshot) -> BuildReceipt {
    BuildReceipt {
        snapshot_id: s.id.clone(),
        engine: ENGINE.into(),
        input_hash: "fixture".into(),
        graph_hash: s.graph_hash.clone(),
        topology_hash: s.topology_hash.clone(),
        validated: true,
        stats: BuildStats::default(),
    }
}

#[test]
fn deletion_and_new_symbol_refresh_unchanged_references() {
    let (_dir, repo, store) = setup();
    let a = commit(
        &repo,
        &[
            ("lib.py", Some("def target():\n    return 1\n")),
            (
                "main.py",
                Some("from lib import target, later\ndef run():\n    return target() + later()\n"),
            ),
        ],
    );
    let mut e = Engine::new(repo.workdir().unwrap(), store).unwrap();
    let (sa, _) = build(&mut e, &a, false);
    let b = commit(&repo, &[("lib.py", Some("def later():\n    return 2\n"))]);
    let (sb, _) = build(&mut e, &b, false);
    let (cold, _) = build(&mut e, &b, true);
    assert_eq!(sb.graph_hash, cold.graph_hash);
    assert_ne!(sa.edges, sb.edges);
    assert!(sb
        .edges
        .iter()
        .all(|edge| sb.modules.contains_key(&edge.from) && sb.modules.contains_key(&edge.to)));
}

#[test]
fn compare_sources_cosmetic_and_deleted_entities() {
    let (_dir, repo, store) = setup();
    let a = commit(
        &repo,
        &[(
            "unicode.py",
            Some("# café\r\ndef stay():\r\n    return 'é'\r\n\r\ndef gone():\r\n    return 3\r\n"),
        )],
    );
    let mut e = Engine::new(repo.workdir().unwrap(), store.clone()).unwrap();
    let (sa, _) = build(&mut e, &a, false);
    let b = commit(
        &repo,
        &[(
            "unicode.py",
            Some("# café\r\n# shifted\r\ndef stay():\r\n    # comment\r\n    return 'é'\r\n"),
        )],
    );
    let (sb, _) = build(&mut e, &b, false);
    let c = compare::compare(&sa, &sb, &store).unwrap();
    let stay = module(&sb, "stay");
    let change = c.changes.iter().find(|c| c.id == stay.id).unwrap();
    assert!(change.flags.contains(&"cosmetic".into()));
    assert!(!change.flags.contains(&"modified".into()));
    let gone = module(&sa, "gone");
    let d = query::source_diff(&c, &sa, &sb, &store, Some(&gone.id), None).unwrap();
    assert!(d.after.is_none());
    assert!(d.before.unwrap().text.contains("def gone"));
    let source = query::source(&sb, &store, "", Some(&stay.id)).unwrap();
    assert!(source.text.contains("\r\n"));
    let anchor = source.anchor.unwrap();
    let bytes = source.text.as_bytes();
    assert!(
        std::str::from_utf8(&bytes[anchor.start_byte.unwrap()..anchor.end_byte.unwrap()])
            .unwrap()
            .starts_with("def stay")
    );
}

#[test]
fn unique_move_and_ambiguous_duplicates() {
    let (_dir, repo, store) = setup();
    let a = commit(
        &repo,
        &[
            ("a.py", Some("def a():\n    return 17\n")),
            ("b.py", Some("def b():\n    return 17\n")),
            (
                "unique.py",
                Some("def unique():\n    return 'unique body'\n"),
            ),
        ],
    );
    let mut e = Engine::new(repo.workdir().unwrap(), store.clone()).unwrap();
    let (sa, _) = build(&mut e, &a, false);
    let b = commit(
        &repo,
        &[
            ("a.py", None),
            ("b.py", None),
            ("c.py", Some("def c():\n    return 17\n")),
            ("d.py", Some("def d():\n    return 17\n")),
            ("unique.py", None),
            (
                "moved.py",
                Some("def unique():\n    return 'unique body'\n"),
            ),
        ],
    );
    let (sb, _) = build(&mut e, &b, false);
    let c = compare::compare(&sa, &sb, &store).unwrap();
    assert_eq!(
        c.correspondence.get(&module(&sa, "unique").id),
        Some(&module(&sb, "unique").id)
    );
    assert!(c.diagnostics.iter().any(|d| d.code == "match/ambiguous"));
    assert!(!c.correspondence.contains_key(&module(&sa, "a").id));
}

#[test]
fn graph_aggregation_cycles_and_side_specific_paths() {
    let (_dir, repo, store) = setup();
    let a = commit(
        &repo,
        &[
            ("a.py", Some("from b import b\ndef a():\n    return b()\n")),
            ("b.py", Some("from a import a\ndef b():\n    return a()\n")),
        ],
    );
    let mut e = Engine::new(repo.workdir().unwrap(), store.clone()).unwrap();
    let (sa, _) = build(&mut e, &a, false);
    let view = query::graph_view(
        &sa,
        None,
        None,
        &GraphRequest {
            snapshot: Some(sa.id.clone()),
            ..Default::default()
        },
    )
    .unwrap();
    assert!(!view.edges.is_empty());
    assert_eq!(
        view.edges.iter().map(|e| e.count).sum::<usize>(),
        view.edges
            .iter()
            .map(|e| e.relationships.len())
            .sum::<usize>()
    );
    let all: BTreeSet<_> = view
        .edges
        .iter()
        .flat_map(|e| e.relationships.iter().map(|r| r.edge.id.clone()))
        .collect();
    assert_eq!(all.len(), sa.edges.len());
    let route = query::shortest_path(
        &sa,
        &PathRequest {
            snapshot: sa.id.clone(),
            from: module(&sa, "a").id.clone(),
            to: module(&sa, "b").id.clone(),
            edge_types: vec![],
        },
    )
    .unwrap();
    assert_eq!(route["reachable"], true);
    let b = commit(&repo, &[("a.py", Some("def a():\n    return 1\n"))]);
    let (sb, _) = build(&mut e, &b, false);
    let c = compare::compare(&sa, &sb, &store).unwrap();
    let union = query::graph_view(
        &sb,
        Some(&sa),
        Some(&c),
        &GraphRequest {
            comparison: Some(c.id.clone()),
            ..Default::default()
        },
    )
    .unwrap();
    assert!(union.edges.iter().any(|e| e.removed > 0));
    let route = query::shortest_path(
        &sb,
        &PathRequest {
            snapshot: sb.id.clone(),
            from: module(&sb, "a").id.clone(),
            to: module(&sb, "b").id.clone(),
            edge_types: vec![],
        },
    )
    .unwrap();
    assert_eq!(route["reachable"], false);
    let limited = query::graph_view(
        &sa,
        None,
        None,
        &GraphRequest {
            snapshot: Some(sa.id.clone()),
            limit: Some(1),
            ..Default::default()
        },
    )
    .unwrap();
    assert!(limited.truncated);
    assert_eq!(limited.nodes.len(), 1);
}

#[test]
fn corruption_rebuilds_and_config_changes_invalidate() {
    let (_dir, repo, store) = setup();
    let a = commit(
        &repo,
        &[
            ("custom.xyz", Some("def f():\n    return 1\n")),
            (".semrc", Some(".xyz = python\n")),
            ("broken.py", Some("def broken(:\n    pass\n")),
            ("notes.unknown", Some("unrecognized format")),
        ],
    );
    let mut e = Engine::new(repo.workdir().unwrap(), store.clone()).unwrap();
    let (sa, r) = build(&mut e, &a, false);
    store.publish(&sa, &r).unwrap();
    assert!(sa.diagnostics.iter().any(|d| d.code == "parse/recovered"));
    assert!(sa
        .diagnostics
        .iter()
        .any(|d| d.code == "coverage/unsupported"));
    let key = sa.files["custom.xyz"].object.as_ref().unwrap();
    fs::write(
        store.root.join("objects").join(&key[..2]).join(&key[2..]),
        "corrupt",
    )
    .unwrap();
    assert!(store.get(key).is_err());
    let (repaired, _) = build(&mut e, &a, false);
    assert_eq!(sa.graph_hash, repaired.graph_hash);
    assert!(store.get(key).is_ok());
    let b = commit(&repo, &[(".semrc", Some(".xyz = javascript\n"))]);
    let (sb, r) = build(&mut e, &b, false);
    assert_eq!(r.stats.mode, "cold");
    assert_ne!(sa.config_hash, sb.config_hash);
    let (cold, _) = build(&mut e, &b, true);
    assert_eq!(cold.graph_hash, sb.graph_hash);
}

#[test]
fn parent_suppression_and_file_only_changes() {
    let (_dir, repo, store) = setup();
    let a = commit(
        &repo,
        &[(
            "m.py",
            Some("import os\nclass Box:\n    def value(self):\n        return 1\n"),
        )],
    );
    let mut e = Engine::new(repo.workdir().unwrap(), store.clone()).unwrap();
    let (sa, _) = build(&mut e, &a, false);
    let b = commit(
        &repo,
        &[(
            "m.py",
            Some("import sys\nclass Box:\n    def value(self):\n        return 2\n"),
        )],
    );
    let (sb, _) = build(&mut e, &b, false);
    let c = compare::compare(&sa, &sb, &store).unwrap();
    let class = module(&sb, "Box");
    let change = c.changes.iter().find(|c| c.id == class.id).unwrap();
    assert!(
        !change.headline,
        "parent should be suppressed when only its method changes"
    );
    assert!(c
        .changes
        .iter()
        .find(|c| c.id == file_id("m.py"))
        .unwrap()
        .flags
        .contains(&"modified".into()));
}

#[test]
fn snapshot_identifiers_do_not_merge_equal_functions() {
    assert_ne!(
        module_id("a.py::function::f"),
        module_id("b.py::function::f")
    );
}

#[test]
fn multiple_languages_and_bare_repository() {
    let (dir, repo, store) = setup();
    let sha = commit(
        &repo,
        &[
            ("a.py", Some("def f():\n    return 1\n")),
            ("b.ts", Some("export function f(): number { return 1; }")),
            ("c.rs", Some("pub fn f() -> i32 { 1 }")),
            ("d.go", Some("package d\nfunc F() int { return 1 }")),
            ("e.json", Some("{\"name\":\"fixture\"}")),
            ("f.yaml", Some("name: fixture\n")),
        ],
    );
    let bare = git2::build::RepoBuilder::new()
        .bare(true)
        .clone(repo.path().to_str().unwrap(), &dir.path().join("bare.git"))
        .unwrap();
    let mut e = Engine::new(bare.path(), store).unwrap();
    let (s, _) = build(&mut e, &sha, false);
    for p in ["a.py", "b.ts", "c.rs", "d.go", "e.json", "f.yaml"] {
        assert!(
            s.modules.values().any(|m| m.path == p && m.is_entity()),
            "missing {p}"
        );
    }
}

#[test]
fn chunk_reuse_branches_skipped_commits_and_qualified_search() {
    let (_dir, repo, store) = setup();
    let a = commit(
        &repo,
        &[
            (
                "stable.py",
                Some("class Stable:\n    def value(self):\n        return 42\n"),
            ),
            ("changed.py", Some("def first():\n    return 1\n")),
        ],
    );
    let mut engine = Engine::new(repo.workdir().unwrap(), store.clone()).unwrap();
    let (sa, ra) = build(&mut engine, &a, false);
    store.publish(&sa, &ra).unwrap();
    let ka = store.put_snapshot(&sa).unwrap();
    let ma: xfold::store::SnapshotManifest = store.get_json(&ka).unwrap();
    let _b = commit(
        &repo,
        &[("changed.py", Some("def middle():\n    return 2\n"))],
    );
    let c = commit(
        &repo,
        &[("changed.py", Some("def last():\n    return 3\n"))],
    );
    let (sc, rc) = build(&mut engine, &c, false);
    assert_eq!(rc.stats.files_parsed, 1);
    let mc: xfold::store::SnapshotManifest =
        store.get_json(&store.put_snapshot(&sc).unwrap()).unwrap();
    assert_eq!(ma.module_chunks["stable.py"], mc.module_chunks["stable.py"]);
    assert_ne!(
        ma.module_chunks["changed.py"],
        mc.module_chunks["changed.py"]
    );
    let (cold, _) = build(&mut engine, &c, true);
    assert_eq!(sc.graph_hash, cold.graph_hash);
    // Build the earlier commit through the resident worker, bypassing snapshot
    // lookup by using a separate index entry removal; no cold reset of sem.
    store
        .connection()
        .unwrap()
        .execute("DELETE FROM snapshots WHERE id=?1", [&sa.id])
        .unwrap();
    let (back, rb) = build(&mut engine, &a, false);
    assert_eq!(rb.stats.mode, "incremental");
    assert_eq!(back.graph_hash, sa.graph_hash);
    repo.set_head_detached(git2::Oid::from_str(&a).unwrap())
        .unwrap();
    // Set the index to the branch parent so the fixture tree is independent.
    repo.index()
        .unwrap()
        .read_tree(
            &repo
                .find_commit(git2::Oid::from_str(&a).unwrap())
                .unwrap()
                .tree()
                .unwrap(),
        )
        .unwrap();
    let d = commit(
        &repo,
        &[("changed.py", Some("def branch():\n    return 4\n"))],
    );
    let (sd, _) = build(&mut engine, &d, false);
    let (dcold, _) = build(&mut engine, &d, true);
    assert_eq!(sd.graph_hash, dcold.graph_hash);
    store.publish(&sd, &build_receipt(&sd)).unwrap();
    let name = &module(&sd, "value").qualified_name;
    let (items, _) = store.search(&sd, name, None, None, None, (0, 10)).unwrap();
    assert!(items.iter().any(|m| m.name == "value"));
}

#[test]
fn imports_reordering_and_missing_git_objects() {
    let (_dir, repo, store) = setup();
    let a=commit(&repo,&[("a.py",Some("def one():\n    return 1\n")),("b.py",Some("def two():\n    return 2\n")),("main.py",Some("from a import one as value\ndef first():\n    return value()\ndef second():\n    return 2\n"))]);
    let mut engine = Engine::new(repo.workdir().unwrap(), store.clone()).unwrap();
    let (sa, ra) = build(&mut engine, &a, false);
    store.publish(&sa, &ra).unwrap();
    let b=commit(&repo,&[("main.py",Some("from b import two as value\ndef second():\n    return 2\ndef first():\n    return value()\n"))]);
    let (sb, _) = build(&mut engine, &b, false);
    let (cold, _) = build(&mut engine, &b, true);
    assert_eq!(sb.graph_hash, cold.graph_hash);
    let diff = compare::compare(&sa, &sb, &store).unwrap();
    assert!(diff
        .changes
        .iter()
        .any(|c| c.flags.contains(&"reordered".into())));
    let c = commit(
        &repo,
        &[("nested/missing.py", Some("def missing():\n    pass\n"))],
    );
    let tree = repo
        .find_commit(git2::Oid::from_str(&c).unwrap())
        .unwrap()
        .tree()
        .unwrap();
    let oid = tree.get_name("nested").unwrap().id().to_string();
    fs::remove_file(repo.path().join("objects").join(&oid[..2]).join(&oid[2..])).unwrap();
    assert!(engine
        .build(
            &BuildRequest {
                revision: c,
                profile: Profile::default(),
                force: false
            },
            |_, _| {}
        )
        .is_err());
    assert_eq!(store.snapshot(&sa.id).unwrap().graph_hash, sa.graph_hash);
}

#[test]
fn overload_reordering_and_ambiguous_reused_sem_ids() {
    let (_dir, repo, store) = setup();
    let a = commit(&repo, &[("Overload.java", Some("class Overload {\n int f(int x) { return x + 1; }\n String f(String x) { return x + \"!\"; }\n}\n")), ("duplicates.py", Some("def f():\n    return 11\n\ndef f():\n    return 11\n"))]);
    let mut engine = Engine::new(repo.workdir().unwrap(), store.clone()).unwrap();
    let (base, _) = build(&mut engine, &a, false);
    let b = commit(&repo, &[("Overload.java", Some("class Overload {\n String f(String x) { return x + \"!\"; }\n int f(int x) { return x + 1; }\n}\n")), ("duplicates.py", Some("def f():\n    return 12\n\ndef f():\n    return 12\n"))]);
    let (target, _) = build(&mut engine, &b, false);
    let comparison = compare::compare(&base, &target, &store).unwrap();
    let overloads: Vec<_> = base
        .modules
        .values()
        .filter(|m| m.name == "f" && m.path == "Overload.java")
        .collect();
    assert_eq!(overloads.len(), 2);
    for old in overloads {
        let next = &target.modules[&comparison.correspondence[&old.id]];
        assert_eq!(
            old.content_hash, next.content_hash,
            "Overloads must follow their signatures/bodies, not their old line numbers"
        );
    }
    let view = query::graph_view(
        &target,
        Some(&base),
        Some(&comparison),
        &GraphRequest {
            comparison: Some(comparison.id.clone()),
            scope: Some(file_id("duplicates.py")),
            ..Default::default()
        },
    )
    .unwrap();
    assert_eq!(view.nodes.len(), 4);
    assert_eq!(
        view.nodes
            .iter()
            .map(|n| &n.module.id)
            .collect::<BTreeSet<_>>()
            .len(),
        4
    );
    let deleted = view
        .nodes
        .iter()
        .find(|n| n.flags.contains(&"deleted".into()))
        .unwrap();
    let diff = query::source_diff(
        &comparison,
        &base,
        &target,
        &store,
        Some(&deleted.module.id),
        None,
    )
    .unwrap();
    assert!(diff.before.unwrap().text.contains("return 11"));
    assert!(diff.after.is_none());
    assert!(comparison.summary.get("deleted").copied().unwrap_or(0) >= 2);
}

#[test]
fn light_projection_excludes_unrelated_siblings_and_keeps_deleted_context() {
    let (_dir, repo, store) = setup();
    let a = commit(&repo, &[
        ("src/service.py", Some("class Service:\n    def changed(self):\n        return 1\n\n    def sibling(self):\n        return 2\n\ndef removed():\n    return 3\n")),
        ("unrelated.py", Some("def unrelated():\n    return 0\n")),
        ("notes.txt", Some("before\n")),
    ]);
    let mut engine = Engine::new(repo.workdir().unwrap(), store.clone()).unwrap();
    let (base, _) = build(&mut engine, &a, false);
    let b = commit(&repo, &[
        ("src/service.py", Some("class Service:\n    def changed(self):\n        return 4\n\n    def sibling(self):\n        return 2\n")),
        ("notes.txt", Some("after\n")),
    ]);
    let (target, _) = build(&mut engine, &b, false);
    let c = compare::compare(&base, &target, &store).unwrap();
    let request = GraphRequest {
        light: true,
        comparison: Some(c.id.clone()),
        ..Default::default()
    };
    let view = query::graph_view(&target, Some(&base), Some(&c), &request).unwrap();
    let names: BTreeSet<_> = view.nodes.iter().map(|n| n.module.name.as_str()).collect();
    assert!(
        names.contains("changed")
            && names.contains("Service")
            && names.contains("service.py")
            && names.contains("src")
    );
    assert!(names.contains("removed") && names.contains("notes.txt"));
    assert!(
        !names.contains("sibling")
            && !names.contains("unrelated")
            && !names.contains("unrelated.py")
    );
    assert!(view
        .nodes
        .iter()
        .any(|n| n.module.name == "removed" && n.side == "base"));
    assert!(!view.truncated);
    let expanded = GraphRequest {
        expanded: view.nodes.iter().map(|n| n.module.id.clone()).collect(),
        ..request.clone()
    };
    let expanded = query::graph_view(&target, Some(&base), Some(&c), &expanded).unwrap();
    assert_eq!(view.nodes.len(), expanded.nodes.len());
    let limited = query::graph_view(
        &target,
        Some(&base),
        Some(&c),
        &GraphRequest {
            limit: Some(1),
            ..request
        },
    )
    .unwrap();
    assert!(limited.truncated);
    assert_eq!(limited.omitted_nodes, view.nodes.len() - 1);
    assert!(query::change_seeds(&c, &target, &base)
        .iter()
        .any(|c| c.id == limited.nodes[0].module.id));
    let unchanged = compare::compare(&target, &target, &store).unwrap();
    assert!(query::graph_view(
        &target,
        Some(&target),
        Some(&unchanged),
        &GraphRequest {
            light: true,
            ..Default::default()
        }
    )
    .unwrap()
    .nodes
    .is_empty());
}

#[test]
fn light_root_commit_and_direct_child_context() {
    let (_dir, repo, store) = setup();
    let a = commit(
        &repo,
        &[(
            "lib.py",
            Some("class Example:\n    def child(self):\n        return 1\n"),
        )],
    );
    let mut engine = Engine::new(repo.workdir().unwrap(), store.clone()).unwrap();
    let (base, receipt) = build(&mut engine, xfold::repo::EMPTY_TREE, false);
    store.publish(&base, &receipt).unwrap();
    assert!(base.files.is_empty());
    let (target, _) = build(&mut engine, &a, false);
    let mut c = compare::compare(&base, &target, &store).unwrap();
    let view = query::graph_view(
        &target,
        Some(&base),
        Some(&c),
        &GraphRequest {
            light: true,
            ..Default::default()
        },
    )
    .unwrap();
    assert!(view
        .nodes
        .iter()
        .any(|n| n.module.name == "Example" && n.flags.contains(&"added".into())));
    // A changed class includes its direct child even if sem suppresses that child.
    for change in &mut c.changes {
        change.headline = change.id == module(&target, "Example").id;
    }
    let view = query::graph_view(
        &target,
        Some(&base),
        Some(&c),
        &GraphRequest {
            light: true,
            ..Default::default()
        },
    )
    .unwrap();
    assert!(view.nodes.iter().any(|n| n.module.name == "child"));
}
