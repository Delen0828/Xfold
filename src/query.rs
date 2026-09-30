use crate::{model::*, store::*};
use anyhow::{bail, Context, Result};
use std::collections::{BTreeMap, BTreeSet, VecDeque};

pub fn ancestors(modules: &BTreeMap<String, Module>, id: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut next = modules.get(id).and_then(|m| m.parent.as_deref());
    while let Some(p) = next {
        if out.iter().any(|x| x == p) {
            break;
        }
        out.push(p.to_owned());
        next = modules.get(p).and_then(|m| m.parent.as_deref());
    }
    out
}

/// Semantic headlines are the seeds, not containers whose hashes changed merely
/// because a descendant changed. Keep file-only edits (including unsupported text).
pub fn change_seeds<'a>(
    comparison: &'a Comparison,
    target: &Snapshot,
    base: &Snapshot,
) -> Vec<&'a Change> {
    let module = |c: &Change| {
        c.after
            .as_ref()
            .and_then(|id| target.modules.get(id))
            .or_else(|| c.before.as_ref().and_then(|id| base.modules.get(id)))
    };
    let meaningful = |c: &Change| c.flags.iter().any(|f| f != "location");
    let entity_paths: BTreeSet<_> = comparison
        .changes
        .iter()
        .filter(|c| c.headline && meaningful(c))
        .filter_map(|c| module(c).map(|m| m.path.clone()))
        .collect();
    comparison
        .changes
        .iter()
        .filter(|c| {
            meaningful(c)
                && (c.headline
                    || module(c)
                        .is_some_and(|m| m.kind == "file" && !entity_paths.contains(&m.path)))
        })
        .collect()
}

/// Immutable hierarchy aggregates shared by every projection of one artifact.
pub struct ProjectionIndex {
    modules: BTreeMap<String, Module>,
    base_ids: BTreeMap<String, String>,
    change_by_id: BTreeMap<String, Change>,
    children: BTreeMap<String, Vec<String>>,
    parents: BTreeMap<String, Vec<String>>,
    descendants: BTreeMap<String, usize>,
    changed_descendants: BTreeMap<String, usize>,
    moved_out: BTreeMap<String, usize>,
    light_ids: BTreeSet<String>,
    seeds: BTreeSet<String>,
}
impl ProjectionIndex {
    pub fn new(
        target: &Snapshot,
        base: Option<&Snapshot>,
        comparison: Option<&Comparison>,
    ) -> Self {
        let base_ids: BTreeMap<String, String> = comparison
            .map(|c| {
                c.changes
                    .iter()
                    .filter_map(|change| {
                        change
                            .before
                            .as_ref()
                            .map(|id| (id.clone(), change.id.clone()))
                    })
                    .collect()
            })
            .unwrap_or_default();
        let mut modules = target.modules.clone();
        let mut change_by_id = BTreeMap::new();
        if let (Some(base), Some(c)) = (base, comparison) {
            for change in &c.changes {
                change_by_id.insert(change.id.clone(), change.clone());
                if change.after.is_none() {
                    let mut m = base.modules[change.before.as_ref().unwrap()].clone();
                    m.id = change.id.clone();
                    m.parent = m
                        .parent
                        .as_ref()
                        .map(|p| base_ids.get(p).cloned().unwrap_or_else(|| p.clone()));
                    modules.insert(m.id.clone(), m);
                }
            }
        }
        let mut children: BTreeMap<String, Vec<String>> = BTreeMap::new();
        let mut parents: BTreeMap<String, Vec<String>> = BTreeMap::new();
        let mut descendants: BTreeMap<String, usize> = BTreeMap::new();
        let mut changed_descendants: BTreeMap<String, usize> = BTreeMap::new();
        let mut moved_out: BTreeMap<String, usize> = BTreeMap::new();
        for m in modules.values() {
            if let Some(p) = &m.parent {
                children.entry(p.clone()).or_default().push(m.id.clone());
            }
            let ancestors = ancestors(&modules, &m.id);
            let changed = change_by_id
                .get(&m.id)
                .is_some_and(|c| c.flags.iter().any(|f| f != "location"));
            for p in &ancestors {
                *descendants.entry(p.clone()).or_default() += 1;
                if changed {
                    *changed_descendants.entry(p.clone()).or_default() += 1;
                }
            }
            parents.insert(m.id.clone(), ancestors);
        }
        if let (Some(base), Some(c)) = (base, comparison) {
            for change in c
                .changes
                .iter()
                .filter(|c| c.flags.iter().any(|f| f == "moved"))
            {
                if let Some(b) = &change.before {
                    for p in ancestors(&base.modules, b) {
                        let p = base_ids.get(&p).cloned().unwrap_or(p);
                        *moved_out.entry(p).or_default() += 1;
                    }
                }
            }
        }
        let mut light_ids = BTreeSet::new();
        let mut seeds = BTreeSet::new();
        if let (Some(base), Some(c)) = (base, comparison) {
            for change in change_seeds(c, target, base) {
                seeds.insert(change.id.clone());
                light_ids.insert(change.id.clone());
                light_ids.extend(parents.get(&change.id).into_iter().flatten().cloned());
                light_ids.extend(children.get(&change.id).into_iter().flatten().cloned());
                // Moves retain both the old and new containing hierarchy.
                if let Some(before) = &change.before {
                    light_ids.extend(
                        ancestors(&base.modules, before)
                            .into_iter()
                            .filter_map(|id| base_ids.get(&id).cloned()),
                    );
                }
            }
        }
        Self {
            light_ids,
            seeds,
            modules,
            base_ids,
            change_by_id,
            children,
            parents,
            descendants,
            changed_descendants,
            moved_out,
        }
    }
}

pub fn graph_view(
    target: &Snapshot,
    base: Option<&Snapshot>,
    comparison: Option<&Comparison>,
    req: &GraphRequest,
) -> Result<GraphView> {
    graph_view_indexed(
        target,
        base,
        comparison,
        req,
        &ProjectionIndex::new(target, base, comparison),
    )
}

pub fn graph_view_indexed(
    target: &Snapshot,
    base: Option<&Snapshot>,
    comparison: Option<&Comparison>,
    req: &GraphRequest,
    index: &ProjectionIndex,
) -> Result<GraphView> {
    let ProjectionIndex {
        modules,
        base_ids,
        change_by_id,
        children,
        parents,
        descendants,
        changed_descendants,
        moved_out,
        light_ids,
        seeds,
    } = index;
    if req.light && comparison.is_none() {
        bail!("A light graph requires a commit comparison");
    }
    let scope = req.scope.as_deref().unwrap_or(ROOT);
    if !modules.contains_key(scope) {
        bail!("Unknown graph scope {scope}");
    }
    let expanded: BTreeSet<_> = req.expanded.iter().cloned().collect();
    let mut visible = BTreeSet::new();
    let mut todo = VecDeque::from(
        children
            .get(scope)
            .cloned()
            .unwrap_or_else(|| vec![scope.into()]),
    );
    while let Some(id) = todo.pop_front() {
        if !visible.insert(id.clone()) {
            continue;
        }
        if expanded.contains(&id) {
            if let Some(cs) = children.get(&id) {
                todo.extend(cs.iter().cloned());
            }
        }
    }
    if req.light {
        // Open directly on the whole change neighborhood. Scope can narrow it,
        // but expansion must never pull unrelated siblings into this projection.
        visible = light_ids
            .iter()
            .filter(|id| {
                id.as_str() != ROOT
                    && (scope == ROOT
                        || id.as_str() == scope
                        || parents
                            .get(*id)
                            .is_some_and(|ps| ps.iter().any(|p| p == scope)))
            })
            .cloned()
            .collect();
    }
    visible.retain(|id| {
        let m = &modules[id];
        (req.kinds.is_empty() || !m.is_entity() || req.kinds.contains(&m.kind))
            && (req.changes.is_empty()
                || change_by_id
                    .get(id)
                    .is_some_and(|c| c.flags.iter().any(|f| req.changes.contains(f)))
                || changed_descendants.get(id).copied().unwrap_or(0) > 0)
    });
    let limit = req.limit.unwrap_or(300).clamp(1, 1000);
    let total = visible.len();
    let mut ordered: Vec<_> = visible.into_iter().collect();
    ordered.sort_by_key(|id| {
        (
            req.light && !seeds.contains(id),
            &modules[id].path,
            modules[id].source.as_ref().map(|s| s.start_line),
            id.clone(),
        )
    });
    ordered.truncate(limit);
    let mut visible: BTreeSet<_> = ordered.into_iter().collect();
    let scope_ancestors = ancestors(modules, scope);
    let mut boundary = BTreeSet::new();
    let relations = if let Some(c) = comparison {
        c.edges.clone()
    } else {
        target
            .edges
            .iter()
            .map(|e| EdgeChange {
                edge: e.clone(),
                status: "unchanged".into(),
                side: "target".into(),
            })
            .collect()
    };
    let display_id = |id: &str, side: &str| -> String {
        if side == "base" {
            base_ids.get(id).cloned().unwrap_or_else(|| id.into())
        } else {
            id.into()
        }
    };
    let project = |id: &str, visible: &BTreeSet<String>| -> Option<String> {
        if visible.contains(id) {
            return Some(id.into());
        }
        parents
            .get(id)?
            .iter()
            .find(|p| visible.contains(*p))
            .cloned()
    };
    let boundary_for = |id: &str| -> Option<String> {
        let chain = parents.get(id)?;
        if id == scope || chain.iter().any(|p| p == scope) {
            return None;
        }
        std::iter::once(id.to_owned())
            .chain(chain.iter().cloned())
            .find(|p| {
                modules[p]
                    .parent
                    .as_ref()
                    .is_some_and(|q| scope_ancestors.contains(q))
                    && p != ROOT
            })
    };
    // Only add boundary nodes for relationships connected to visible in-scope nodes.
    for relation in relations.iter().filter(|_| !req.light) {
        if !req.edge_types.is_empty() && !req.edge_types.contains(&relation.edge.kind) {
            continue;
        }
        let f = display_id(&relation.edge.from, &relation.side);
        let t = display_id(&relation.edge.to, &relation.side);
        if project(&f, &visible).is_some() {
            if let Some(b) = boundary_for(&t) {
                boundary.insert(b);
            }
        }
        if project(&t, &visible).is_some() {
            if let Some(b) = boundary_for(&f) {
                boundary.insert(b);
            }
        }
    }
    let boundary_total = boundary.len();
    boundary = boundary
        .into_iter()
        .take(limit.saturating_sub(visible.len()))
        .collect();
    visible.extend(boundary.iter().cloned());
    let mut aggregate: BTreeMap<(String, String, String), ViewEdge> = BTreeMap::new();
    let mut omitted_edges = 0;
    for relation in relations {
        if !req.edge_types.is_empty() && !req.edge_types.contains(&relation.edge.kind) {
            continue;
        }
        let f = display_id(&relation.edge.from, &relation.side);
        let t = display_id(&relation.edge.to, &relation.side);
        if req.light && (!light_ids.contains(&f) || !light_ids.contains(&t)) {
            continue;
        }
        let (Some(from), Some(to)) = (project(&f, &visible), project(&t, &visible)) else {
            omitted_edges += 1;
            continue;
        };
        if from == to && f != t {
            continue;
        }
        let key = (from.clone(), to.clone(), relation.edge.kind.clone());
        let edge = aggregate.entry(key.clone()).or_insert(ViewEdge {
            id: digest(&key)?,
            from,
            to,
            kind: relation.edge.kind.clone(),
            count: 0,
            added: 0,
            removed: 0,
            unchanged: 0,
            relationships: Vec::new(),
        });
        edge.count += 1;
        match relation.status.as_str() {
            "added" => edge.added += 1,
            "removed" => edge.removed += 1,
            _ => edge.unchanged += 1,
        };
        edge.relationships.push(relation);
    }
    let mut nodes = Vec::new();
    for id in visible {
        let change = change_by_id.get(&id);
        let deleted = change.is_some_and(|c| c.after.is_none());
        nodes.push(ViewNode {
            module: modules[&id].clone(),
            snapshot: if deleted {
                base.unwrap().id.clone()
            } else {
                target.id.clone()
            },
            side: if deleted { "base" } else { "target" }.into(),
            before: change.and_then(|c| c.before.clone()),
            flags: change.map(|c| c.flags.clone()).unwrap_or_default(),
            child_count: children
                .get(&id)
                .map(|v| {
                    v.iter()
                        .filter(|id| !req.light || light_ids.contains(*id))
                        .count()
                })
                .unwrap_or(0),
            descendant_count: descendants.get(&id).copied().unwrap_or(0),
            changed_descendants: changed_descendants.get(&id).copied().unwrap_or(0),
            moved_out: moved_out.get(&id).copied().unwrap_or(0),
            boundary: boundary.contains(&id),
        });
    }
    nodes.sort_by(|a, b| {
        (
            &a.module.path,
            a.module.source.as_ref().map(|s| s.start_line),
            &a.module.id,
        )
            .cmp(&(
                &b.module.path,
                b.module.source.as_ref().map(|s| s.start_line),
                &b.module.id,
            ))
    });
    let edges: Vec<_> = aggregate.into_values().collect();
    let mut breadcrumbs: Vec<_> = scope_ancestors
        .iter()
        .rev()
        .filter_map(|id| modules.get(id).cloned())
        .collect();
    breadcrumbs.push(modules[scope].clone());
    let layout_hash = digest(&(
        "elk-layered-v1",
        scope,
        nodes
            .iter()
            .map(|n| (&n.module.id, &n.module.name, &n.module.kind, n.boundary))
            .collect::<Vec<_>>(),
        edges
            .iter()
            .map(|e| (&e.from, &e.to, &e.kind))
            .collect::<Vec<_>>(),
    ))?;
    let omitted_nodes = total.saturating_sub(limit) + boundary_total.saturating_sub(boundary.len());
    Ok(GraphView {
        id: digest(&(req, &target.id, comparison.map(|c| &c.id)))?,
        snapshot: target.id.clone(),
        comparison: comparison.map(|c| c.id.clone()),
        scope: scope.into(),
        breadcrumbs,
        nodes,
        edges,
        layout_hash,
        truncated: omitted_nodes > 0,
        omitted_nodes,
        omitted_edges,
    })
}

pub fn source(
    snapshot: &Snapshot,
    store: &Store,
    path: &str,
    module: Option<&str>,
) -> Result<SourceResponse> {
    let anchor = module
        .map(|id| {
            snapshot
                .modules
                .get(id)
                .context("Unknown module")
                .and_then(|m| m.source.clone().context("This container has no source"))
        })
        .transpose()?;
    let path = anchor.as_ref().map(|s| s.path.as_str()).unwrap_or(path);
    let file = snapshot
        .files
        .get(path)
        .context("File is not part of this snapshot")?;
    let blob = file
        .object
        .as_ref()
        .with_context(|| format!("Source is unavailable for {} ({})", path, file.status))?;
    Ok(SourceResponse {
        snapshot: snapshot.id.clone(),
        path: path.into(),
        blob: blob.clone(),
        language: file.language.clone(),
        text: String::from_utf8(store.get(blob)?)?,
        anchor,
    })
}
pub fn source_diff(
    c: &Comparison,
    base: &Snapshot,
    target: &Snapshot,
    store: &Store,
    id: Option<&str>,
    path: Option<&str>,
) -> Result<SourceDiff> {
    let change = id.and_then(|id| {
        c.changes.iter().find(|x| {
            x.id == id || x.before.as_deref() == Some(id) || x.after.as_deref() == Some(id)
        })
    });
    let before = if let Some(change) = change {
        change
            .before
            .as_ref()
            .map(|id| source(base, store, "", Some(id)))
            .transpose()?
    } else {
        path.filter(|p| base.files.contains_key(*p))
            .map(|p| source(base, store, p, None))
            .transpose()?
    };
    let after = if let Some(change) = change {
        change
            .after
            .as_ref()
            .map(|id| source(target, store, "", Some(id)))
            .transpose()?
    } else {
        path.filter(|p| target.files.contains_key(*p))
            .map(|p| source(target, store, p, None))
            .transpose()?
    };
    if before.is_none() && after.is_none() {
        bail!("No source for this comparison selection");
    }
    let old = before.as_ref().map(|s| s.text.as_str()).unwrap_or("");
    let new = after.as_ref().map(|s| s.text.as_str()).unwrap_or("");
    let unified = similar::TextDiff::from_lines(old, new)
        .unified_diff()
        .context_radius(3)
        .header(
            before
                .as_ref()
                .map(|s| s.path.as_str())
                .unwrap_or("/dev/null"),
            after
                .as_ref()
                .map(|s| s.path.as_str())
                .unwrap_or("/dev/null"),
        )
        .to_string();
    let before_name = change
        .and_then(|c| c.before.as_ref())
        .and_then(|id| base.modules.get(id))
        .map(|m| m.name.clone());
    let after_name = change
        .and_then(|c| c.after.as_ref())
        .and_then(|id| target.modules.get(id))
        .map(|m| m.name.clone());
    Ok(SourceDiff {
        comparison: c.id.clone(),
        before_name,
        after_name,
        before,
        after,
        unified,
        flags: change.map(|c| c.flags.clone()).unwrap_or_default(),
    })
}

pub fn traversal(s: &Snapshot, r: &TraverseRequest) -> Result<serde_json::Value> {
    if !s.modules.contains_key(&r.start) {
        bail!("Unknown start module");
    }
    let direction = r.direction.as_deref().unwrap_or("dependencies");
    if !["dependencies", "dependents"].contains(&direction) {
        bail!("Direction must be dependencies or dependents");
    }
    let reverse = direction == "dependents";
    let limit = r.limit.unwrap_or(300).clamp(1, 10000);
    let depth = r.depth.unwrap_or(1).min(100);
    let mut adjacency: BTreeMap<&str, Vec<(&str, &Dependency)>> = BTreeMap::new();
    for e in &s.edges {
        if r.edge_types.is_empty() || r.edge_types.contains(&e.kind) {
            let (a, b) = if reverse {
                (&e.to, &e.from)
            } else {
                (&e.from, &e.to)
            };
            adjacency.entry(a).or_default().push((b, e));
        }
    }
    let mut queue = VecDeque::from([(r.start.clone(), 0)]);
    let mut seen = BTreeSet::from([r.start.clone()]);
    let mut edges = BTreeMap::new();
    let mut truncated = false;
    while let Some((id, d)) = queue.pop_front() {
        if d >= depth {
            continue;
        }
        for (next, e) in adjacency.get(id.as_str()).into_iter().flatten() {
            if !seen.contains(*next) && seen.len() >= limit {
                truncated = true;
                continue;
            }
            edges.insert(e.id.clone(), (*e).clone());
            if seen.insert((*next).into()) {
                queue.push_back(((*next).into(), d + 1));
            }
        }
    }
    Ok(
        serde_json::json!({"snapshot":s.id,"direction":direction,"nodes":seen.into_iter().map(|id|s.modules[&id].clone()).collect::<Vec<_>>(),"edges":edges.into_values().collect::<Vec<_>>(),"truncated":truncated}),
    )
}
pub fn shortest_path(s: &Snapshot, r: &PathRequest) -> Result<serde_json::Value> {
    if !s.modules.contains_key(&r.from) || !s.modules.contains_key(&r.to) {
        bail!("Unknown path endpoint");
    }
    let mut adjacency: BTreeMap<&str, Vec<&Dependency>> = BTreeMap::new();
    for e in &s.edges {
        if r.edge_types.is_empty() || r.edge_types.contains(&e.kind) {
            adjacency.entry(&e.from).or_default().push(e);
        }
    }
    for v in adjacency.values_mut() {
        v.sort_by(|a, b| (&a.to, &a.kind).cmp(&(&b.to, &b.kind)));
    }
    let mut queue = VecDeque::from([r.from.clone()]);
    let mut seen = BTreeSet::from([r.from.clone()]);
    let mut prev = BTreeMap::new();
    while let Some(id) = queue.pop_front() {
        if id == r.to {
            break;
        }
        for e in adjacency.get(id.as_str()).into_iter().flatten() {
            if seen.insert(e.to.clone()) {
                prev.insert(e.to.clone(), (*e).clone());
                queue.push_back(e.to.clone());
            }
        }
    }
    if !seen.contains(&r.to) {
        return Ok(serde_json::json!({"snapshot":s.id,"reachable":false,"nodes":[],"edges":[]}));
    }
    let mut ids = vec![r.to.clone()];
    let mut edges = Vec::new();
    let mut current = r.to.clone();
    while current != r.from {
        let e = prev[&current].clone();
        current = e.from.clone();
        ids.push(current.clone());
        edges.push(e);
    }
    ids.reverse();
    edges.reverse();
    Ok(serde_json::json!({"snapshot":s.id,"reachable":true,"nodes":ids,"edges":edges}))
}
