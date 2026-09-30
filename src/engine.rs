use crate::{model::*, repo, store::*};
use anyhow::{bail, Context, Result};
use fs2::FileExt;
use ignore::gitignore::GitignoreBuilder;
use sem_core::parser::{
    facts_store::FactsStore, plugins::create_default_registry, registry::ParserRegistry,
    session::GraphSession,
};
use sem_core::utils::scan::{is_default_excluded, is_probably_binary_path};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::{Path, PathBuf},
    time::Instant,
};

pub struct Engine {
    repository: PathBuf,
    pub store: Store,
    materialized: PathBuf,
    session: Option<GraphSession>,
    compatibility: Option<String>,
    previous_blobs: BTreeMap<String, String>,
    previous_diagnostics: Vec<Diagnostic>,
    _lock: fs::File,
}

impl Engine {
    pub fn new(repository: &Path, store: Store) -> Result<Self> {
        let lock = fs::OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(store.root.join("worker.lock"))?;
        lock.try_lock_exclusive().context("Another Xfold build worker owns this cache; use a different --cache-dir or stop that process")?;
        let materialized = store.root.join("materialized");
        fs::create_dir_all(&materialized)?;
        Ok(Self {
            repository: repository.to_owned(),
            store,
            materialized,
            session: None,
            compatibility: None,
            previous_blobs: BTreeMap::new(),
            previous_diagnostics: Vec::new(),
            _lock: lock,
        })
    }

    pub fn build(
        &mut self,
        request: &BuildRequest,
        mut progress: impl FnMut(&str, usize),
    ) -> Result<(Snapshot, BuildReceipt)> {
        let started = Instant::now();
        let git = repo::open(&self.repository)?;
        let commit = repo::resolve(&git, &request.revision)?;
        let repository = repo::identity(&git);
        let mut profile = request.profile.clone();
        profile.extensions.sort();
        profile.extensions.dedup();
        let id = snapshot_id(&repository, &commit, &profile)?;
        if !request.force {
            if let Ok(Some(s)) = self.store.load_snapshot(&id) {
                // A snapshot hit does no parsing. Verify source objects so corrupt
                // content cannot be served under an apparently healthy receipt.
                let valid = s
                    .files
                    .values()
                    .filter_map(|f| f.object.as_ref())
                    .all(|k| self.store.get(k).is_ok());
                if valid {
                    let mut receipt = self.store.receipt(&id)?;
                    receipt.stats = BuildStats {
                        mode: "cache-hit".into(),
                        duration_ms: started.elapsed().as_millis() as u64,
                        ..Default::default()
                    };
                    return Ok((s, receipt));
                }
            }
        }
        progress("reading", 0);
        let (tree, entries) = repo::tree_files(&git, &commit)?;
        clean_materialization(
            &self.materialized,
            &self.materialized,
            &entries.keys().cloned().collect(),
        )?;
        let mut configs = BTreeMap::new();
        for name in [".semrc", ".gitattributes", ".semignore"] {
            let path = self.materialized.join(name);
            if let Some(entry) = entries
                .get(name)
                .filter(|e| e.mode == 0o100644 || e.mode == 0o100755)
            {
                let blob = git.find_blob(entry.oid)?;
                fs::write(&path, blob.content())?;
                configs.insert(name.to_string(), hash(blob.content()));
            } else if path.exists() {
                fs::remove_file(path)?;
            }
        }
        let config_hash = digest(&(ENGINE, &profile, &configs))?;
        let dependency_fingerprint = dependency_fingerprint();
        let compatibility_hash = digest(&(&config_hash, &dependency_fingerprint))?;
        let mut registry = create_default_registry();
        registry.load_semrc(&self.materialized);
        registry.load_gitattributes(&self.materialized);
        let mut ignore = GitignoreBuilder::new(&self.materialized);
        if entries.contains_key(".semignore") {
            if let Some(e) = ignore.add(self.materialized.join(".semignore")) {
                bail!("Invalid committed .semignore: {e}");
            }
        }
        let ignore = ignore.build()?;
        let compatible = self.compatibility.as_ref() == Some(&compatibility_hash) && !request.force;
        let mut files = BTreeMap::new();
        let mut diagnostics = Vec::new();
        let mut blobs = BTreeMap::new();
        let mut changed = Vec::new();
        let mut syntax_to_check = Vec::new();
        for (path, entry) in &entries {
            let mode_ok = entry.mode == 0o100644 || entry.mode == 0o100755;
            let hidden = path.split('/').any(|p| p.starts_with('.'));
            let excluded = (!profile.include_excluded && (is_default_excluded(path) || hidden))
                || ignore.matched_path_or_any_parents(path, false).is_ignore();
            let mut record = FileRecord {
                path: path.clone(),
                git_blob: entry.oid.to_string(),
                object: None,
                language: "plaintext".into(),
                status: "unsupported".into(),
                bytes: entry.size,
                lines: 0,
            };
            if !mode_ok {
                record.status = if entry.mode == 0o160000 {
                    "submodule"
                } else {
                    "symlink"
                }
                .into();
            } else if is_probably_binary_path(path) {
                record.status = "binary".into();
            } else if entry.size > 4 * 1024 * 1024 {
                record.status = "too-large".into();
            } else {
                let blob = git.find_blob(entry.oid)?;
                if let Some(text) = std::str::from_utf8(blob.content())
                    .ok()
                    .filter(|s| !s.contains('\0'))
                {
                    record.object = Some(self.store.put(blob.content())?);
                    record.lines = text.split('\n').count();
                    let detection = registry
                        .resolve_file_path(path)
                        .unwrap_or_else(|| path.clone());
                    let plugin = registry.get_plugin_with_content(&detection, text);
                    record.language = language_for(&registry, &detection, text);
                    let extension_ok = profile.extensions.is_empty()
                        || profile.extensions.iter().any(|ext| path.ends_with(ext));
                    if excluded || !extension_ok {
                        record.status = "excluded".into();
                    } else if plugin.is_some_and(|p| p.id() != "fallback") {
                        record.status = "analyzed".into();
                        blobs.insert(path.clone(), entry.oid.to_string());
                        let dest = self.materialized.join(path);
                        let dirty = !compatible
                            || self.previous_blobs.get(path) != Some(&entry.oid.to_string());
                        if dirty || !dest.exists() {
                            fs::create_dir_all(dest.parent().unwrap())?;
                            fs::write(&dest, blob.content())?;
                            changed.push(path.clone());
                            syntax_to_check.push(path.clone());
                        }
                    }
                } else {
                    record.status = "non-text".into();
                }
            }
            if record.status != "analyzed" {
                diagnostics.push(Diagnostic::warning(
                    &format!("coverage/{}", record.status),
                    format!(
                        "{} is not included in semantic analysis ({})",
                        path, record.status
                    ),
                    Some(path.clone()),
                ));
            }
            files.insert(path.clone(), record);
        }
        // Remove stale materialized paths, including leftovers after a crash.
        let allowed: BTreeSet<_> = blobs
            .keys()
            .cloned()
            .chain(configs.keys().cloned())
            .collect();
        clean_materialization(&self.materialized, &self.materialized, &allowed)?;
        for path in self.previous_blobs.keys() {
            if !blobs.contains_key(path) {
                changed.push(path.clone());
            }
        }
        changed.sort();
        changed.dedup();
        let paths: Vec<_> = blobs.keys().cloned().collect();
        let facts = FactsStore::open(self.store.root.join("facts").join(&compatibility_hash));
        progress("parsing", paths.len());
        let mut mode;
        if let Some(session) = self.session.as_mut().filter(|_| compatible) {
            mode = "incremental";
            session.rebuild(&paths, &changed, &registry);
        } else if !request.force {
            if let Some(loaded) = facts.load(&self.materialized) {
                mode = "warm-start";
                self.session =
                    Some(GraphSession::warm_start(&self.materialized, &paths, &registry, loaded).0);
            } else {
                mode = "cold";
                self.session = Some(GraphSession::build(&self.materialized, &paths, &registry));
            }
        } else {
            mode = "cold";
            self.session = Some(GraphSession::build(&self.materialized, &paths, &registry));
        }
        // sem's pinned incremental resolver can retain a deleted import target.
        // Reject that candidate and repair via the cold oracle before publication.
        let graph = self.session.as_ref().unwrap().graph();
        if graph.edges.iter().any(|edge| {
            !graph.entities.contains_key(&edge.from_entity)
                || !graph.entities.contains_key(&edge.to_entity)
        }) {
            progress("repairing-resolution", paths.len());
            self.session = Some(GraphSession::build(&self.materialized, &paths, &registry));
            mode = "incremental-fallback";
        }
        // sem tolerates syntax recovery; expose it rather than implying full coverage.
        for path in &syntax_to_check {
            let text = fs::read_to_string(self.materialized.join(path))?;
            let tree = registry
                .extract_entities_with_tree(path, &text)
                .and_then(|(_, tree)| tree);
            if tree.is_some_and(|t| t.root_node().has_error()) {
                diagnostics.push(Diagnostic::warning("parse/recovered", "The parser recovered from syntax errors; entities or relationships may be incomplete", Some(path.clone())));
            }
        }
        if compatible {
            diagnostics.extend(
                self.previous_diagnostics
                    .iter()
                    .filter(|d| {
                        d.code == "parse/recovered"
                            && d.path
                                .as_ref()
                                .is_some_and(|p| blobs.contains_key(p) && !changed.contains(p))
                    })
                    .cloned(),
            );
        }
        let session = self.session.as_ref().unwrap();
        progress("normalizing", session.entities().len());
        let mut modules = BTreeMap::new();
        modules.insert(
            ROOT.into(),
            container(ROOT.into(), "repository", "Repository", "", None, None),
        );
        for file in files.values() {
            let parent = directory_nodes(&file.path, &mut modules);
            let source = file.object.as_ref().map(|blob| SourceRef {
                path: file.path.clone(),
                blob: blob.clone(),
                start_line: 1,
                end_line: file.lines.max(1),
                start_byte: Some(0),
                end_byte: Some(file.bytes),
            });
            let mut m = container(
                file_id(&file.path),
                "file",
                file.path.rsplit('/').next().unwrap(),
                &file.path,
                Some(parent),
                source,
            );
            m.content_hash = file.object.clone().unwrap_or_else(|| file.git_blob.clone());
            m.language = file.language.clone();
            modules.insert(m.id.clone(), m);
        }
        for e in session.entities() {
            let Some(file) = files.get(&e.file_path) else {
                bail!("sem returned entity outside the snapshot: {}", e.file_path);
            };
            let Some(blob) = &file.object else {
                bail!("Entity in non-text file {}", e.file_path);
            };
            let id = module_id(&e.id);
            let m = Module {
                id: id.clone(),
                sem_id: Some(e.id.clone()),
                kind: e.entity_type.clone(),
                name: e.name.clone(),
                qualified_name: e.id.clone(),
                parent: Some(
                    e.parent_id
                        .as_ref()
                        .map(|p| module_id(p))
                        .unwrap_or_else(|| file_id(&e.file_path)),
                ),
                path: e.file_path.clone(),
                language: file.language.clone(),
                source: Some(SourceRef {
                    path: e.file_path.clone(),
                    blob: blob.clone(),
                    start_line: e.start_line.max(1),
                    end_line: e.end_line.max(e.start_line).max(1),
                    start_byte: e.start_byte,
                    end_byte: e.end_byte,
                }),
                content_hash: hash(e.content.as_bytes()),
                structural_hash: e.structural_hash.clone(),
                kappa: e.kappa.clone(),
            };
            if modules.insert(id, m).is_some() {
                bail!("Duplicate entity ID from sem: {}", e.id);
            }
        }
        // sem sometimes emits graph-only entities, such as top-level references.
        for (sem_id, info) in &session.graph().entities {
            let id = module_id(sem_id.as_str());
            if modules.contains_key(&id) {
                continue;
            }
            if let Some(file) = files.get(&info.file_path) {
                let source = file.object.as_ref().map(|blob| SourceRef {
                    path: info.file_path.clone(),
                    blob: blob.clone(),
                    start_line: info.start_line.max(1),
                    end_line: info.end_line.max(info.start_line).max(1),
                    start_byte: None,
                    end_byte: None,
                });
                modules.insert(
                    id.clone(),
                    Module {
                        id,
                        sem_id: Some(sem_id.to_string()),
                        kind: info.entity_type.clone(),
                        name: info.name.clone(),
                        qualified_name: sem_id.to_string(),
                        parent: Some(
                            info.parent_id
                                .as_ref()
                                .map(|p| module_id(p.as_str()))
                                .unwrap_or_else(|| file_id(&info.file_path)),
                        ),
                        path: info.file_path.clone(),
                        language: file.language.clone(),
                        source,
                        content_hash: String::new(),
                        structural_hash: None,
                        kappa: None,
                    },
                );
            }
        }
        let known: BTreeSet<_> = modules.keys().cloned().collect();
        for m in modules.values_mut() {
            if m.parent.as_ref().is_some_and(|p| !known.contains(p)) {
                diagnostics.push(Diagnostic::warning(
                    "entity/missing-parent",
                    format!(
                        "Attached {} to its file because sem did not extract its parent",
                        m.name
                    ),
                    Some(m.path.clone()),
                ));
                m.parent = Some(file_id(&m.path));
            }
        }
        let mut edges = Vec::new();
        for e in &session.graph().edges {
            let from = module_id(e.from_entity.as_str());
            let to = module_id(e.to_entity.as_str());
            if !modules.contains_key(&from) || !modules.contains_key(&to) {
                bail!(
                    "sem dependency has missing endpoint: {} → {}",
                    e.from_entity,
                    e.to_entity
                );
            }
            let kind = serde_json::to_value(&e.ref_type)?
                .as_str()
                .unwrap()
                .to_owned();
            edges.push(Dependency {
                id: digest(&(&from, &to, &kind))?,
                from,
                to,
                kind,
                provenance: "sem/static-reference; call-site unavailable".into(),
            });
        }
        edges.sort();
        edges.dedup();
        diagnostics
            .sort_by(|a, b| (&a.path, &a.code, &a.message).cmp(&(&b.path, &b.code, &b.message)));
        let source_hash = digest(
            &files
                .values()
                .map(|f| (&f.path, &f.git_blob))
                .collect::<Vec<_>>(),
        )?;
        let topology_hash = digest(&(
            modules
                .values()
                .map(|m| (&m.id, &m.parent, &m.kind))
                .collect::<Vec<_>>(),
            &edges,
        ))?;
        let graph_hash = digest(&(
            SCHEMA,
            ENGINE,
            &config_hash,
            &modules,
            &edges,
            &files,
            &diagnostics,
        ))?;
        let snapshot = Snapshot {
            id: id.clone(),
            schema_version: SCHEMA,
            engine: ENGINE.into(),
            dependency_fingerprint,
            repository,
            commit,
            tree,
            profile,
            config_hash: config_hash.clone(),
            source_hash,
            graph_hash: graph_hash.clone(),
            topology_hash: topology_hash.clone(),
            modules,
            edges,
            files,
            diagnostics,
        };
        progress("validating", snapshot.modules.len());
        validate(&snapshot, &self.store)?;
        let stats = session.stats();
        let receipt = BuildReceipt {
            snapshot_id: id,
            engine: ENGINE.into(),
            input_hash: digest(&(
                &snapshot.tree,
                &snapshot.profile,
                &config_hash,
                &snapshot.dependency_fingerprint,
            ))?,
            graph_hash,
            topology_hash,
            validated: true,
            stats: BuildStats {
                mode: mode.into(),
                duration_ms: started.elapsed().as_millis() as u64,
                files_changed: changed.len(),
                files_parsed: stats.files_seed_red,
                files_reused: stats.files_green,
                files_resolved: stats.files_red,
                files_deleted: stats.files_deleted,
                edges_reused: stats.edges_reused,
                edges_rederived: stats.edges_rederived,
                artifact_bytes: serde_json::to_vec(&snapshot)?.len(),
            },
        };
        facts.save(&self.materialized, &session.export_persisted())?;
        self.previous_blobs = blobs;
        self.compatibility = Some(compatibility_hash);
        self.previous_diagnostics = snapshot.diagnostics.clone();
        // Publication is owned by the coordinator; the worker only supplies a candidate.
        Ok((snapshot, receipt))
    }
}

fn clean_materialization(root: &Path, dir: &Path, allowed: &BTreeSet<String>) -> Result<()> {
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        if entry.file_type()?.is_dir() {
            clean_materialization(root, &path, allowed)?;
            if fs::read_dir(&path)?.next().is_none() {
                fs::remove_dir(path)?;
            }
        } else {
            let rel = path
                .strip_prefix(root)?
                .to_string_lossy()
                .replace('\\', "/");
            if !allowed.contains(&rel) {
                fs::remove_file(path)?;
            }
        }
    }
    Ok(())
}
fn container(
    id: String,
    kind: &str,
    name: &str,
    path: &str,
    parent: Option<String>,
    source: Option<SourceRef>,
) -> Module {
    Module {
        id,
        sem_id: None,
        kind: kind.into(),
        name: name.into(),
        qualified_name: path.into(),
        parent,
        path: path.into(),
        language: String::new(),
        source,
        content_hash: String::new(),
        structural_hash: None,
        kappa: None,
    }
}
fn directory_nodes(path: &str, modules: &mut BTreeMap<String, Module>) -> String {
    let mut parent = ROOT.to_owned();
    let mut parts = Vec::new();
    let pieces: Vec<_> = path.split('/').collect();
    for part in &pieces[..pieces.len() - 1] {
        parts.push(*part);
        let path = parts.join("/");
        let id = dir_id(&path);
        modules.entry(id.clone()).or_insert_with(|| {
            container(
                id.clone(),
                "directory",
                part,
                &path,
                Some(parent.clone()),
                None,
            )
        });
        parent = id;
    }
    parent
}
fn language_for(registry: &ParserRegistry, path: &str, text: &str) -> String {
    let ext = Path::new(path)
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("");
    let lang = match ext {
        "py" | "pyi" => "python",
        "rs" => "rust",
        "ts" | "tsx" | "mts" | "cts" => "typescript",
        "js" | "jsx" | "mjs" | "cjs" => "javascript",
        "h" | "c" => "c",
        "cpp" | "cc" | "cxx" | "hpp" => "cpp",
        "yml" | "yaml" => "yaml",
        "md" | "mdx" => "markdown",
        "sh" | "bash" => "shell",
        "svg" | "xml" => "xml",
        "cs" => "csharp",
        "rb" => "ruby",
        "tex" => "latex",
        "tf" | "hcl" => "hcl",
        x if !x.is_empty() => x,
        _ => registry
            .get_plugin_with_content(path, text)
            .map(|p| p.id())
            .unwrap_or("plaintext"),
    };
    lang.into()
}
pub fn validate(snapshot: &Snapshot, store: &Store) -> Result<()> {
    for m in snapshot.modules.values() {
        let mut seen = BTreeSet::new();
        let mut next = Some(m.id.as_str());
        while let Some(id) = next {
            if !seen.insert(id) {
                bail!("Containment cycle at {id}");
            }
            next = snapshot
                .modules
                .get(id)
                .context("Missing containment parent")?
                .parent
                .as_deref();
        }
        if let Some(s) = &m.source {
            let file = snapshot.files.get(&s.path).context("Missing source file")?;
            if file.object.as_ref() != Some(&s.blob)
                || s.start_line == 0
                || s.end_line < s.start_line
                || s.end_line > file.lines.max(1)
            {
                bail!(
                    "Invalid source range for {}: {}..{} / {}",
                    m.name,
                    s.start_line,
                    s.end_line,
                    file.lines
                );
            }
            if let (Some(start), Some(end)) = (s.start_byte, s.end_byte) {
                if start > end || end > file.bytes {
                    bail!("Invalid byte span for {}", m.name);
                }
            }
        }
    }
    for e in &snapshot.edges {
        if !snapshot.modules.contains_key(&e.from) || !snapshot.modules.contains_key(&e.to) {
            bail!("Dangling dependency {}", e.id);
        }
    }
    for f in snapshot.files.values() {
        if let Some(k) = &f.object {
            store.get(k)?;
        }
    }
    Ok(())
}
