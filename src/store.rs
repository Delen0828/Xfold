use crate::model::*;
use anyhow::{bail, Context, Result};
use rusqlite::{params, Connection, OptionalExtension};
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::{
    fs,
    path::{Path, PathBuf},
    time::Duration,
};

/// File-sized immutable records keep unchanged facts shared between revisions.
/// Adjacency is chunked by source file, never recursively by dependency target.
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SnapshotManifest {
    pub format: String,
    pub snapshot: Snapshot,
    pub module_chunks: BTreeMap<String, String>,
    pub edge_chunks: BTreeMap<String, String>,
}

pub fn hash(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}
pub fn digest<T: Serialize + ?Sized>(data: &T) -> Result<String> {
    Ok(hash(&serde_json::to_vec(data)?))
}
pub fn module_id(sem_id: &str) -> String {
    format!("m-{}", &hash(sem_id.as_bytes())[..32])
}
pub fn file_id(path: &str) -> String {
    format!("f-{}", &hash(path.as_bytes())[..32])
}
pub fn dir_id(path: &str) -> String {
    if path.is_empty() {
        ROOT.into()
    } else {
        format!("d-{}", &hash(path.as_bytes())[..32])
    }
}
pub fn snapshot_id(repository: &str, commit: &str, profile: &Profile) -> Result<String> {
    digest(&(
        SCHEMA,
        ENGINE,
        dependency_fingerprint(),
        repository,
        commit,
        profile,
    ))
}
/// Grammar/resolver dependency changes cannot reuse analysis from an older build.
/// This is deliberately separate from hashes of normalized semantic output.
pub fn dependency_fingerprint() -> String {
    hash(include_bytes!("../Cargo.lock"))
}

#[derive(Clone)]
pub struct Store {
    pub root: PathBuf,
}
impl Store {
    pub fn open(root: impl Into<PathBuf>) -> Result<Self> {
        let s = Self { root: root.into() };
        fs::create_dir_all(s.root.join("objects"))?;
        let c = s.connection()?;
        c.pragma_update(None, "journal_mode", "WAL")?;
        c.execute_batch("CREATE TABLE IF NOT EXISTS snapshots(id TEXT PRIMARY KEY, commit_id TEXT NOT NULL, object TEXT NOT NULL, receipt TEXT NOT NULL);
            CREATE TABLE IF NOT EXISTS modules(snapshot TEXT NOT NULL, id TEXT NOT NULL, parent TEXT, name TEXT NOT NULL, path TEXT NOT NULL, kind TEXT NOT NULL, language TEXT NOT NULL, start_line INTEGER, end_line INTEGER, PRIMARY KEY(snapshot,id));
            CREATE INDEX IF NOT EXISTS module_parent ON modules(snapshot,parent);
            CREATE INDEX IF NOT EXISTS module_source ON modules(snapshot,path,start_line,end_line);
            CREATE TABLE IF NOT EXISTS edges(snapshot TEXT NOT NULL,id TEXT NOT NULL,source TEXT NOT NULL,target TEXT NOT NULL,kind TEXT NOT NULL,PRIMARY KEY(snapshot,id));
            CREATE INDEX IF NOT EXISTS edge_source ON edges(snapshot,source);
            CREATE INDEX IF NOT EXISTS edge_target ON edges(snapshot,target);
            CREATE VIRTUAL TABLE IF NOT EXISTS search USING fts5(snapshot UNINDEXED,id UNINDEXED,name,path,kind,language,tokenize='unicode61');
            CREATE TABLE IF NOT EXISTS comparisons(id TEXT PRIMARY KEY,object TEXT NOT NULL);")?;
        Ok(s)
    }
    pub fn connection(&self) -> Result<Connection> {
        let c = Connection::open(self.root.join("index.sqlite"))?;
        c.busy_timeout(Duration::from_secs(30))?;
        Ok(c)
    }
    fn object_path(&self, key: &str) -> Result<PathBuf> {
        if key.len() != 64 || !key.bytes().all(|b| b.is_ascii_hexdigit()) {
            bail!("Invalid object hash");
        }
        Ok(self.root.join("objects").join(&key[..2]).join(&key[2..]))
    }
    pub fn put(&self, bytes: &[u8]) -> Result<String> {
        let key = hash(bytes);
        let path = self.object_path(&key)?;
        // Verify existing objects, so an interrupted/corrupt cache is repaired.
        if fs::read(&path).is_ok_and(|b| hash(&b) == key) {
            return Ok(key);
        }
        fs::create_dir_all(path.parent().unwrap())?;
        let tmp = path.with_extension(format!("{}.tmp", std::process::id()));
        fs::write(&tmp, bytes)?;
        fs::File::open(&tmp)?.sync_all()?;
        if path.exists() {
            fs::remove_file(&path)?;
        }
        fs::rename(&tmp, &path)?;
        Ok(key)
    }
    pub fn get(&self, key: &str) -> Result<Vec<u8>> {
        let bytes = fs::read(self.object_path(key)?)
            .with_context(|| format!("Missing cache object {key}; rebuild this snapshot"))?;
        if hash(&bytes) != key {
            bail!("Corrupt cache object {key}; rebuild this snapshot");
        }
        Ok(bytes)
    }
    pub fn put_json<T: Serialize>(&self, value: &T) -> Result<String> {
        self.put(&serde_json::to_vec(value)?)
    }
    pub fn get_json<T: DeserializeOwned>(&self, key: &str) -> Result<T> {
        Ok(serde_json::from_slice(&self.get(key)?)?)
    }
    pub fn put_snapshot(&self, snapshot: &Snapshot) -> Result<String> {
        let mut modules: BTreeMap<&str, Vec<&Module>> = BTreeMap::new();
        let mut edges: BTreeMap<&str, Vec<&Dependency>> = BTreeMap::new();
        for module in snapshot.modules.values() {
            modules.entry(&module.path).or_default().push(module);
        }
        for edge in &snapshot.edges {
            edges
                .entry(&snapshot.modules[&edge.from].path)
                .or_default()
                .push(edge);
        }
        let module_chunks = modules
            .into_iter()
            .map(|(path, rows)| Ok((path.to_owned(), self.put_json(&rows)?)))
            .collect::<Result<_>>()?;
        let edge_chunks = edges
            .into_iter()
            .map(|(path, rows)| Ok((path.to_owned(), self.put_json(&rows)?)))
            .collect::<Result<_>>()?;
        let mut metadata = snapshot.clone();
        metadata.modules.clear();
        metadata.edges.clear();
        self.put_json(&SnapshotManifest {
            format: "xfold/snapshot-chunks/1".into(),
            snapshot: metadata,
            module_chunks,
            edge_chunks,
        })
    }
    pub fn get_snapshot(&self, key: &str) -> Result<Snapshot> {
        let value: serde_json::Value = self.get_json(key)?;
        // Read early v1 caches as well; all new writes use chunk manifests.
        if value.get("format").is_none() {
            return Ok(serde_json::from_value(value)?);
        }
        let manifest: SnapshotManifest = serde_json::from_value(value)?;
        if manifest.format != "xfold/snapshot-chunks/1" {
            bail!("Unsupported snapshot manifest format");
        }
        let mut snapshot = manifest.snapshot;
        for key in manifest.module_chunks.values() {
            for module in self.get_json::<Vec<Module>>(key)? {
                if snapshot.modules.insert(module.id.clone(), module).is_some() {
                    bail!("Duplicate entity in snapshot manifest");
                }
            }
        }
        for key in manifest.edge_chunks.values() {
            snapshot
                .edges
                .extend(self.get_json::<Vec<Dependency>>(key)?);
        }
        snapshot.edges.sort();
        Ok(snapshot)
    }
    pub fn load_snapshot(&self, id: &str) -> Result<Option<Snapshot>> {
        self.snapshot_key(id)?
            .map(|k| self.get_snapshot(&k))
            .transpose()
    }
    pub fn snapshot_key(&self, id: &str) -> Result<Option<String>> {
        Ok(self
            .connection()?
            .query_row("SELECT object FROM snapshots WHERE id=?1", [id], |r| {
                r.get(0)
            })
            .optional()?)
    }
    pub fn snapshot(&self, id: &str) -> Result<Snapshot> {
        self.load_snapshot(id)?
            .with_context(|| format!("Snapshot {id} is not ready"))
    }
    pub fn receipt(&self, id: &str) -> Result<BuildReceipt> {
        let key: String = self.connection()?.query_row(
            "SELECT receipt FROM snapshots WHERE id=?1",
            [id],
            |r| r.get(0),
        )?;
        self.get_json(&key)
    }
    pub fn list(&self) -> Result<Vec<serde_json::Value>> {
        let c = self.connection()?;
        let mut q = c.prepare("SELECT id,commit_id FROM snapshots ORDER BY rowid DESC")?;
        let rows = q.query_map([], |r| {
            Ok(serde_json::json!({"id": r.get::<_,String>(0)?, "commit": r.get::<_,String>(1)?}))
        })?;
        Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
    }
    pub fn publish(&self, snapshot: &Snapshot, receipt: &BuildReceipt) -> Result<()> {
        let object = self.put_snapshot(snapshot)?;
        let receipt = self.put_json(receipt)?;
        let mut c = self.connection()?;
        let tx = c.transaction()?;
        tx.execute("DELETE FROM modules WHERE snapshot=?1", [&snapshot.id])?;
        tx.execute("DELETE FROM edges WHERE snapshot=?1", [&snapshot.id])?;
        tx.execute("DELETE FROM search WHERE snapshot=?1", [&snapshot.id])?;
        {
            let mut insert =
                tx.prepare("INSERT INTO modules VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9)")?;
            let mut search = tx.prepare("INSERT INTO search VALUES(?1,?2,?3,?4,?5,?6)")?;
            for m in snapshot.modules.values() {
                insert.execute(params![
                    snapshot.id,
                    m.id,
                    m.parent,
                    m.name,
                    m.path,
                    m.kind,
                    m.language,
                    m.source.as_ref().map(|s| s.start_line),
                    m.source.as_ref().map(|s| s.end_line)
                ])?;
                search.execute(params![
                    snapshot.id,
                    m.id,
                    m.qualified_name,
                    m.path,
                    m.kind,
                    m.language
                ])?;
            }
            let mut edge = tx.prepare("INSERT INTO edges VALUES(?1,?2,?3,?4,?5)")?;
            for e in &snapshot.edges {
                edge.execute(params![snapshot.id, e.id, e.from, e.to, e.kind])?;
            }
        }
        tx.execute(
            "INSERT OR REPLACE INTO snapshots VALUES(?1,?2,?3,?4)",
            params![snapshot.id, snapshot.commit, object, receipt],
        )?;
        tx.commit()?;
        Ok(())
    }
    pub fn save_comparison(&self, comparison: &Comparison) -> Result<()> {
        let key = self.put_json(comparison)?;
        self.connection()?.execute(
            "INSERT OR REPLACE INTO comparisons VALUES(?1,?2)",
            params![comparison.id, key],
        )?;
        Ok(())
    }
    pub fn comparison(&self, id: &str) -> Result<Comparison> {
        let key: String = self
            .connection()?
            .query_row("SELECT object FROM comparisons WHERE id=?1", [id], |r| {
                r.get(0)
            })
            .with_context(|| format!("Comparison {id} is not ready"))?;
        self.get_json(&key)
    }
    pub fn search(
        &self,
        snapshot: &Snapshot,
        term: &str,
        kind: Option<&str>,
        language: Option<&str>,
        path: Option<&str>,
        page: (usize, usize),
    ) -> Result<(Vec<Module>, usize)> {
        let (offset, limit) = page;
        let c = self.connection()?;
        // Literal substring search also finds punctuation-heavy qualified names.
        let pattern = format!(
            "%{}%",
            term.replace('\\', "\\\\")
                .replace('%', "\\%")
                .replace('_', "\\_")
        );
        let mut q = c.prepare("SELECT modules.id FROM modules JOIN search ON modules.snapshot=search.snapshot AND modules.id=search.id WHERE modules.snapshot=?1 AND (search.name LIKE ?2 ESCAPE '\\' OR modules.path LIKE ?2 ESCAPE '\\') AND (?3 IS NULL OR modules.kind=?3) AND (?4 IS NULL OR modules.language=?4) AND (?5 IS NULL OR modules.path LIKE ?5 ESCAPE '\\') ORDER BY modules.path,start_line,modules.id")?;
        let path = path.map(|p| {
            format!(
                "{}%",
                p.replace('\\', "\\\\")
                    .replace('%', "\\%")
                    .replace('_', "\\_")
            )
        });
        let ids = q
            .query_map(params![snapshot.id, pattern, kind, language, path], |r| {
                r.get::<_, String>(0)
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        let total = ids.len();
        Ok((
            ids.into_iter()
                .skip(offset)
                .take(limit)
                .filter_map(|id| snapshot.modules.get(&id).cloned())
                .collect(),
            total,
        ))
    }
}

pub fn cache_root(override_path: Option<&Path>) -> Result<PathBuf> {
    if let Some(p) = override_path {
        return Ok(p.to_path_buf());
    }
    Ok(directories::ProjectDirs::from("dev", "Xfold", "Xfold")
        .context("Cannot find OS cache directory; provide --cache-dir")?
        .cache_dir()
        .to_path_buf())
}
