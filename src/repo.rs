use crate::store::hash;
use anyhow::{bail, Context, Result};
use git2::{ObjectType, Oid, Repository, Sort};
use serde::Serialize;
use std::{
    collections::BTreeMap,
    path::{Component, Path, PathBuf},
};

pub fn open(path: &Path) -> Result<Repository> {
    Repository::discover(path).with_context(|| format!("{} is not a Git repository. Xfold v1 reads committed revisions; provide a local checkout or bare repository.", path.display()))
}
pub fn identity(repo: &Repository) -> String {
    hash(
        repo.path()
            .canonicalize()
            .unwrap_or_else(|_| repo.path().to_owned())
            .to_string_lossy()
            .as_bytes(),
    )
}
pub const EMPTY_TREE: &str = ":empty";

pub fn resolve(repo: &Repository, revision: &str) -> Result<String> {
    if revision == EMPTY_TREE {
        return Ok(EMPTY_TREE.into());
    }
    Ok(repo.revparse_single(revision).with_context(|| format!("Revision {revision:?} is unavailable locally. Fetch it using Git before opening it in Xfold."))?.peel_to_commit()?.id().to_string())
}

pub fn safe_relative(path: &str) -> bool {
    !path.is_empty()
        && !path.contains('\\')
        && !path.contains('\0')
        && !path.contains(':')
        && Path::new(path)
            .components()
            .all(|x| matches!(x, Component::Normal(_)))
        && !path.split('/').any(|x| x.eq_ignore_ascii_case(".git"))
}

#[derive(Clone)]
pub struct BlobEntry {
    pub oid: Oid,
    pub mode: i32,
    pub size: usize,
}

pub fn tree_files(
    repo: &Repository,
    commit: &str,
) -> Result<(String, BTreeMap<String, BlobEntry>)> {
    if commit == EMPTY_TREE {
        return Ok((EMPTY_TREE.into(), BTreeMap::new()));
    }
    let tree = repo.find_commit(Oid::from_str(commit)?)?.tree()?;
    let mut files = BTreeMap::new();
    let mut error = None;
    let walked = tree.walk(git2::TreeWalkMode::PreOrder, |root, entry| {
        let Some(name) = entry.name() else {
            error = Some("Non-UTF-8 Git paths are not supported".to_string());
            return git2::TreeWalkResult::Abort;
        };
        let path = format!("{root}{name}");
        if !safe_relative(&path) {
            error = Some(format!("Unsafe or nonportable Git path: {path}"));
            return git2::TreeWalkResult::Abort;
        }
        match entry.kind() {
            Some(ObjectType::Blob) => match repo.find_blob(entry.id()) {
                Ok(blob) => {
                    files.insert(
                        path,
                        BlobEntry {
                            oid: entry.id(),
                            mode: entry.filemode(),
                            size: blob.size(),
                        },
                    );
                }
                Err(e) => {
                    error = Some(e.to_string());
                    return git2::TreeWalkResult::Abort;
                }
            },
            Some(ObjectType::Commit) => {
                files.insert(
                    path,
                    BlobEntry {
                        oid: entry.id(),
                        mode: entry.filemode(),
                        size: 0,
                    },
                );
            }
            _ => {}
        }
        git2::TreeWalkResult::Ok
    });
    if let Some(e) = error {
        bail!(e);
    }
    walked.context(
        "Cannot enumerate the committed tree; the repository may have missing Git objects",
    )?;
    Ok((tree.id().to_string(), files))
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CommitInfo {
    pub id: String,
    pub subject: String,
    pub author: String,
    pub time: i64,
    pub parents: Vec<String>,
}

pub fn commits(
    repo: &Repository,
    revision: &str,
    offset: usize,
    limit: usize,
) -> Result<Vec<CommitInfo>> {
    let mut walk = repo.revwalk()?;
    walk.set_sorting(Sort::TOPOLOGICAL | Sort::TIME)?;
    walk.push(Oid::from_str(&resolve(repo, revision)?)?)?;
    walk.skip(offset)
        .take(limit)
        .map(|oid| {
            let c = repo.find_commit(oid?)?;
            let author = c.author().name().unwrap_or("").to_owned();
            Ok(CommitInfo {
                id: c.id().to_string(),
                subject: c.summary().unwrap_or("").into(),
                author,
                time: c.time().seconds(),
                parents: c.parent_ids().map(|p| p.to_string()).collect(),
            })
        })
        .collect()
}
pub fn refs(repo: &Repository) -> Result<Vec<serde_json::Value>> {
    let mut refs = Vec::new();
    for r in repo.references()? {
        let r = r?;
        if let Ok(c) = r.peel_to_commit() {
            refs.push(serde_json::json!({"name": r.shorthand(), "fullName": r.name(), "commit": c.id().to_string()}));
        }
    }
    refs.sort_by_key(|r| r["fullName"].as_str().unwrap_or("").to_string());
    Ok(refs)
}
pub fn repository_path(path: &Path) -> Result<PathBuf> {
    Ok(open(path)?.path().canonicalize()?)
}
