use crate::{model::*, store::*};
use anyhow::{bail, Result};
use sem_core::{
    model::{
        entity::SemanticEntity,
        identity::{default_similarity, match_entities},
    },
    parser::differ::suppress_redundant_parents,
};
use std::collections::{BTreeMap, BTreeSet};

pub fn comparison_id(base: &str, target: &str) -> Result<String> {
    digest(&("comparison-v2", ENGINE, base, target))
}

pub fn entity(m: &Module, store: &Store) -> Result<SemanticEntity> {
    let content = if let Some(s) = &m.source {
        let bytes = store.get(&s.blob)?;
        if let (Some(a), Some(b)) = (s.start_byte, s.end_byte) {
            String::from_utf8(bytes.get(a..b).unwrap_or(&[]).to_vec())?
        } else {
            String::from_utf8(bytes)?
                .split('\n')
                .skip(s.start_line.saturating_sub(1))
                .take(s.end_line - s.start_line + 1)
                .collect::<Vec<_>>()
                .join("\n")
        }
    } else {
        String::new()
    };
    // Deserialize so additions to sem's entity model remain an explicit adapter boundary.
    Ok(serde_json::from_value(
        serde_json::json!({"id":m.id,"filePath":m.path,"entityType":m.kind,"name":m.name,"parentId":m.parent,"content":content,"contentHash":m.content_hash,"structuralHash":m.structural_hash,"kappa":m.kappa,"startLine":m.source.as_ref().map(|s|s.start_line).unwrap_or(1),"endLine":m.source.as_ref().map(|s|s.end_line).unwrap_or(1),"startByte":m.source.as_ref().and_then(|s|s.start_byte),"endByte":m.source.as_ref().and_then(|s|s.end_byte)}),
    )?)
}

pub fn compare(base: &Snapshot, target: &Snapshot, store: &Store) -> Result<Comparison> {
    if base.profile != target.profile {
        bail!("Comparisons require the same extraction profile");
    }
    let mut correspondence: BTreeMap<String, String> = base
        .modules
        .keys()
        .filter(|id| target.modules.contains_key(*id))
        .map(|id| (id.clone(), id.clone()))
        .collect();
    let mut diagnostics = Vec::new();
    let changed_files: BTreeSet<_> = base
        .files
        .keys()
        .chain(target.files.keys())
        .filter(|p| {
            base.config_hash != target.config_hash
                || base.files.get(*p).map(|f| &f.git_blob)
                    != target.files.get(*p).map(|f| &f.git_blob)
        })
        .cloned()
        .collect();
    // sem disambiguates overloads with line suffixes. A reused line number
    // must not force correspondence to a different overload after reordering.
    for snapshot in [base, target] {
        let mut groups: BTreeMap<_, Vec<&Module>> = BTreeMap::new();
        for m in snapshot
            .modules
            .values()
            .filter(|m| m.is_entity() && changed_files.contains(&m.path))
        {
            groups
                .entry((&m.path, &m.parent, &m.kind, &m.name))
                .or_default()
                .push(m);
        }
        for members in groups.values().filter(|members| members.len() > 1) {
            for m in members {
                if base
                    .modules
                    .get(&m.id)
                    .zip(target.modules.get(&m.id))
                    .is_some_and(|(b, a)| b.content_hash != a.content_hash)
                {
                    correspondence.remove(&m.id);
                }
            }
        }
    }
    let before: Vec<_> = base
        .modules
        .values()
        .filter(|m| m.is_entity() && changed_files.contains(&m.path))
        .map(|m| Ok((m.id.clone(), entity(m, store)?)))
        .collect::<Result<_>>()?;
    let after: Vec<_> = target
        .modules
        .values()
        .filter(|m| m.is_entity() && changed_files.contains(&m.path))
        .map(|m| Ok((m.id.clone(), entity(m, store)?)))
        .collect::<Result<_>>()?;
    let unmatched_before: Vec<_> = before
        .iter()
        .filter(|(id, _)| !correspondence.contains_key(id))
        .collect();
    let matched_after: BTreeSet<_> = correspondence.values().cloned().collect();
    let unmatched_after: Vec<_> = after
        .iter()
        .filter(|(id, _)| !matched_after.contains(id))
        .collect();
    let mut proposals: Vec<(f64, String, String)> = Vec::new();
    for (bid, b) in &unmatched_before {
        for (aid, a) in &unmatched_after {
            if b.entity_type != a.entity_type {
                continue;
            }
            let score = if (!b.content_hash.is_empty() && b.content_hash == a.content_hash)
                || (b.structural_hash.is_some() && b.structural_hash == a.structural_hash)
            {
                1.0
            } else {
                default_similarity(b, a)
            };
            let threshold =
                if b.file_path == a.file_path && b.name == a.name && b.parent_id == a.parent_id {
                    0.3
                } else {
                    0.8
                };
            if score >= threshold {
                proposals.push((score, bid.clone(), aid.clone()));
            }
        }
    }
    // Require a unique best candidate in both directions. Never use path ordering
    // to turn duplicate-body ambiguity into asserted semantic identity.
    let mut ambiguous = BTreeSet::new();
    for (score, bid, aid) in &proposals {
        let bbest = proposals
            .iter()
            .filter(|(_, b, _)| b == bid)
            .map(|(s, _, _)| *s)
            .fold(0.0_f64, f64::max);
        let abest = proposals
            .iter()
            .filter(|(_, _, a)| a == aid)
            .map(|(s, _, _)| *s)
            .fold(0.0_f64, f64::max);
        if *score != bbest || *score != abest {
            continue;
        }
        let bc = proposals
            .iter()
            .filter(|(s, b, _)| b == bid && *s == bbest)
            .count();
        let ac = proposals
            .iter()
            .filter(|(s, _, a)| a == aid && *s == abest)
            .count();
        if bc != 1 || ac != 1 {
            ambiguous.insert(bid.clone());
            continue;
        }
        let b = &before.iter().find(|(id, _)| id == bid).unwrap().1;
        let a = &after.iter().find(|(id, _)| id == aid).unwrap().1;
        let matched = match_entities(
            std::slice::from_ref(b),
            std::slice::from_ref(a),
            &a.file_path,
            None,
            None,
            None,
        );
        if !matched.changes.iter().any(|c| {
            matches!(
                c.change_type,
                sem_core::model::change::ChangeType::Added
                    | sem_core::model::change::ChangeType::Deleted
            )
        }) {
            correspondence.insert(bid.clone(), aid.clone());
        }
    }
    for id in ambiguous {
        diagnostics.push(Diagnostic::warning(
            "match/ambiguous",
            format!(
                "{} has multiple equally plausible matches; retained as added/deleted",
                base.modules[&id].qualified_name
            ),
            Some(base.modules[&id].path.clone()),
        ));
    }
    // Match file containers only when the entire blob moves uniquely.
    for b in base
        .modules
        .values()
        .filter(|m| m.kind == "file" && !correspondence.contains_key(&m.id))
        .collect::<Vec<_>>()
    {
        let candidates: Vec<_> = target
            .modules
            .values()
            .filter(|a| {
                a.kind == "file"
                    && a.content_hash == b.content_hash
                    && !correspondence.values().any(|id| id == &a.id)
            })
            .collect();
        let source_count = base
            .modules
            .values()
            .filter(|a| {
                a.kind == "file"
                    && a.content_hash == b.content_hash
                    && !correspondence.contains_key(&a.id)
            })
            .count();
        if candidates.len() == 1 && source_count == 1 {
            correspondence.insert(b.id.clone(), candidates[0].id.clone());
        }
    }
    let reverse: BTreeMap<_, _> = correspondence
        .iter()
        .map(|(a, b)| (b.clone(), a.clone()))
        .collect();
    let mut changes = Vec::new();
    for a in target.modules.values() {
        let b = reverse.get(&a.id).and_then(|id| base.modules.get(id));
        let mut flags = Vec::new();
        if let Some(b) = b {
            if b.name != a.name {
                flags.push("renamed".to_string());
            }
            let mapped_parent = b
                .parent
                .as_ref()
                .and_then(|id| correspondence.get(id))
                .or(b.parent.as_ref());
            if b.path != a.path || mapped_parent != a.parent.as_ref() {
                flags.push("moved".to_string());
            }
            if b.content_hash != a.content_hash {
                let cosmetic = (a.kappa.is_some() && a.kappa == b.kappa)
                    || (a.structural_hash.is_some() && a.structural_hash == b.structural_hash);
                flags.push(if cosmetic { "cosmetic" } else { "modified" }.into());
            }
            if b.source
                .as_ref()
                .map(|s| (s.start_line, s.end_line, s.start_byte, s.end_byte))
                != a.source
                    .as_ref()
                    .map(|s| (s.start_line, s.end_line, s.start_byte, s.end_byte))
            {
                flags.push("location".into());
            }
        } else {
            flags.push("added".into());
        }
        changes.push(Change {
            id: a.id.clone(),
            before: b.map(|m| m.id.clone()),
            after: Some(a.id.clone()),
            headline: a.is_entity(),
            flags,
        });
    }
    for b in base
        .modules
        .values()
        .filter(|b| !correspondence.contains_key(&b.id))
    {
        changes.push(Change {
            id: format!("b-{}", b.id),
            before: Some(b.id.clone()),
            after: None,
            flags: vec!["deleted".into()],
            headline: b.is_entity(),
        });
    }
    // Compare ranks only among matched siblings that stay in the same parent.
    let mut old_groups: BTreeMap<String, Vec<&Module>> = BTreeMap::new();
    let mut new_groups: BTreeMap<String, Vec<&Module>> = BTreeMap::new();
    for (bid, aid) in &correspondence {
        let b = &base.modules[bid];
        let a = &target.modules[aid];
        if !a.is_entity() {
            continue;
        }
        let mapped = b.parent.as_ref().and_then(|p| correspondence.get(p));
        if mapped == a.parent.as_ref() {
            old_groups
                .entry(a.parent.clone().unwrap_or_default())
                .or_default()
                .push(b);
            new_groups
                .entry(a.parent.clone().unwrap_or_default())
                .or_default()
                .push(a);
        }
    }
    let ranks = |mut groups: BTreeMap<String, Vec<&Module>>| {
        let mut ranks = BTreeMap::new();
        for ms in groups.values_mut() {
            ms.sort_by_key(|m| {
                (
                    m.source
                        .as_ref()
                        .map(|s| s.start_byte.unwrap_or(s.start_line)),
                    &m.id,
                )
            });
            for (i, m) in ms.iter().enumerate() {
                ranks.insert(m.id.clone(), i);
            }
        }
        ranks
    };
    let old_ranks = ranks(old_groups);
    let new_ranks = ranks(new_groups);
    for c in &mut changes {
        if let (Some(b), Some(a)) = (&c.before, &c.after) {
            if old_ranks.contains_key(b)
                && new_ranks.contains_key(a)
                && old_ranks.get(b) != new_ranks.get(a)
            {
                c.flags.push("reordered".into());
            }
        }
    }
    let target_edges: BTreeSet<_> = target
        .edges
        .iter()
        .map(|e| (e.from.clone(), e.to.clone(), e.kind.clone()))
        .collect();
    let mut old_edges = BTreeSet::new();
    let mut edges = Vec::new();
    for e in &base.edges {
        let from = correspondence
            .get(&e.from)
            .cloned()
            .unwrap_or_else(|| format!("b-{}", e.from));
        let to = correspondence
            .get(&e.to)
            .cloned()
            .unwrap_or_else(|| format!("b-{}", e.to));
        let key = (from.clone(), to.clone(), e.kind.clone());
        old_edges.insert(key.clone());
        if !target_edges.contains(&key) {
            edges.push(EdgeChange {
                edge: e.clone(),
                status: "removed".into(),
                side: "base".into(),
            });
        }
    }
    for e in &target.edges {
        let status = if old_edges.contains(&(e.from.clone(), e.to.clone(), e.kind.clone())) {
            "unchanged"
        } else {
            "added"
        };
        edges.push(EdgeChange {
            edge: e.clone(),
            status: status.into(),
            side: "target".into(),
        });
    }
    let mut touched_base = BTreeSet::new();
    let mut touched_target = BTreeSet::new();
    for e in edges.iter().filter(|e| e.status != "unchanged") {
        let set = if e.side == "base" {
            &mut touched_base
        } else {
            &mut touched_target
        };
        set.insert(e.edge.from.clone());
        set.insert(e.edge.to.clone());
    }
    for c in &mut changes {
        if c.before
            .as_ref()
            .is_some_and(|id| touched_base.contains(id))
            || c.after
                .as_ref()
                .is_some_and(|id| touched_target.contains(id))
        {
            c.flags.push("dependencies".into());
        }
    }
    // Preserve sem's exact parent/child suppression rule for headline counts.
    let before_entities: Vec<_> = before.into_iter().map(|(_, e)| e).collect();
    let after_entities: Vec<_> = after.into_iter().map(|(_, e)| e).collect();
    let mut semantic = Vec::new();
    for c in &changes {
        if !c.flags.iter().any(|f| {
            [
                "added", "deleted", "modified", "cosmetic", "renamed", "moved",
            ]
            .contains(&f.as_str())
        }) {
            continue;
        }
        let b = c
            .before
            .as_ref()
            .and_then(|id| base.modules.get(id))
            .filter(|m| m.is_entity())
            .map(|m| entity(m, store))
            .transpose()?;
        let a = c
            .after
            .as_ref()
            .and_then(|id| target.modules.get(id))
            .filter(|m| m.is_entity())
            .map(|m| entity(m, store))
            .transpose()?;
        if b.is_none() && a.is_none() {
            continue;
        }
        semantic.extend(
            match_entities(
                &b.into_iter().collect::<Vec<_>>(),
                &a.into_iter().collect::<Vec<_>>(),
                "",
                None,
                None,
                None,
            )
            .changes,
        );
    }
    suppress_redundant_parents(&mut semantic, &before_entities, &after_entities);
    let headlines: BTreeSet<_> = semantic.iter().map(|c| c.entity_id.clone()).collect();
    let mut summary = BTreeMap::new();
    for c in &mut changes {
        c.headline = c.headline
            && (headlines.contains(&c.id)
                || c.after.is_none() && c.before.as_ref().is_some_and(|id| headlines.contains(id))
                || c.flags
                    .iter()
                    .any(|f| f == "dependencies" || f == "reordered"));
        if c.headline {
            for flag in &c.flags {
                if flag != "location" {
                    *summary.entry(flag.clone()).or_insert(0) += 1;
                }
            }
        }
    }
    changes.sort_by(|a, b| a.id.cmp(&b.id));
    edges.sort_by(|a, b| (&a.edge.id, &a.side).cmp(&(&b.edge.id, &b.side)));
    Ok(Comparison {
        id: comparison_id(&base.id, &target.id)?,
        base: base.id.clone(),
        target: target.id.clone(),
        correspondence,
        changes,
        edges,
        summary,
        diagnostics,
    })
}
