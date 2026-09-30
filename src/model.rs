use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use utoipa::ToSchema;

pub const SCHEMA: u32 = 1;
pub const ENGINE: &str = "xfold-1/sem-e830de88ee2267c3d93342cc367d00734d376a12";
pub const ROOT: &str = "root";

#[derive(Clone, Debug, Serialize, Deserialize, ToSchema, PartialEq, Eq, Default)]
#[serde(rename_all = "camelCase")]
pub struct Profile {
    #[serde(default)]
    pub include_excluded: bool,
    #[serde(default)]
    pub extensions: Vec<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize, ToSchema, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SourceRef {
    pub path: String,
    pub blob: String,
    pub start_line: usize,
    pub end_line: usize,
    pub start_byte: Option<usize>,
    pub end_byte: Option<usize>,
}

#[derive(Clone, Debug, Serialize, Deserialize, ToSchema, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Module {
    pub id: String,
    pub sem_id: Option<String>,
    pub kind: String,
    pub name: String,
    pub qualified_name: String,
    pub parent: Option<String>,
    pub path: String,
    pub language: String,
    pub source: Option<SourceRef>,
    pub content_hash: String,
    pub structural_hash: Option<String>,
    pub kappa: Option<String>,
}
impl Module {
    pub fn is_entity(&self) -> bool {
        self.sem_id.is_some()
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, ToSchema, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "camelCase")]
pub struct Dependency {
    pub id: String,
    pub from: String,
    pub to: String,
    pub kind: String,
    pub provenance: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, ToSchema, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Diagnostic {
    pub severity: String,
    pub code: String,
    pub message: String,
    pub path: Option<String>,
}
impl Diagnostic {
    pub fn warning(code: &str, message: impl Into<String>, path: Option<String>) -> Self {
        Self {
            severity: "warning".into(),
            code: code.into(),
            message: message.into(),
            path,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, ToSchema, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct FileRecord {
    pub path: String,
    pub git_blob: String,
    pub object: Option<String>,
    pub language: String,
    pub status: String,
    pub bytes: usize,
    pub lines: usize,
}

#[derive(Clone, Debug, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct Snapshot {
    pub id: String,
    pub schema_version: u32,
    pub engine: String,
    #[serde(default)]
    pub dependency_fingerprint: String,
    pub repository: String,
    pub commit: String,
    pub tree: String,
    pub profile: Profile,
    pub config_hash: String,
    pub source_hash: String,
    pub graph_hash: String,
    pub topology_hash: String,
    pub modules: BTreeMap<String, Module>,
    pub edges: Vec<Dependency>,
    pub files: BTreeMap<String, FileRecord>,
    pub diagnostics: Vec<Diagnostic>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct BuildStats {
    pub mode: String,
    pub duration_ms: u64,
    pub files_changed: usize,
    pub files_parsed: usize,
    pub files_reused: usize,
    pub files_resolved: usize,
    pub files_deleted: usize,
    pub edges_reused: usize,
    pub edges_rederived: usize,
    pub artifact_bytes: usize,
}

#[derive(Clone, Debug, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct BuildReceipt {
    pub snapshot_id: String,
    pub engine: String,
    pub input_hash: String,
    pub graph_hash: String,
    pub topology_hash: String,
    pub validated: bool,
    pub stats: BuildStats,
}

#[derive(Clone, Debug, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct Change {
    pub id: String,
    pub before: Option<String>,
    pub after: Option<String>,
    pub flags: Vec<String>,
    pub headline: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct EdgeChange {
    pub edge: Dependency,
    pub status: String,
    pub side: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct Comparison {
    pub id: String,
    pub base: String,
    pub target: String,
    pub correspondence: BTreeMap<String, String>,
    pub changes: Vec<Change>,
    pub edges: Vec<EdgeChange>,
    pub summary: BTreeMap<String, usize>,
    pub diagnostics: Vec<Diagnostic>,
}

#[derive(Clone, Debug, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct BuildRequest {
    pub revision: String,
    #[serde(default)]
    pub profile: Profile,
    #[serde(default)]
    pub force: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct CompareRequest {
    pub base: String,
    pub target: String,
    #[serde(default)]
    pub profile: Profile,
}

#[derive(Clone, Debug, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct Job {
    pub id: String,
    pub state: String,
    pub phase: String,
    pub sequence: u64,
    pub result_id: Option<String>,
    pub result_kind: String,
    pub error: Option<String>,
    pub counters: BTreeMap<String, usize>,
}

#[derive(Clone, Debug, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ReadyOrJob {
    pub state: String,
    pub id: String,
    pub kind: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, ToSchema, Default)]
#[serde(rename_all = "camelCase")]
pub struct GraphRequest {
    pub snapshot: Option<String>,
    pub comparison: Option<String>,
    pub scope: Option<String>,
    #[serde(default)]
    pub expanded: Vec<String>,
    #[serde(default)]
    pub kinds: Vec<String>,
    #[serde(default)]
    pub changes: Vec<String>,
    #[serde(default)]
    pub edge_types: Vec<String>,
    pub limit: Option<usize>,
}

#[derive(Clone, Debug, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ViewNode {
    #[serde(flatten)]
    pub module: Module,
    pub snapshot: String,
    pub side: String,
    pub before: Option<String>,
    pub flags: Vec<String>,
    pub child_count: usize,
    pub descendant_count: usize,
    pub changed_descendants: usize,
    pub moved_out: usize,
    pub boundary: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ViewEdge {
    pub id: String,
    pub from: String,
    pub to: String,
    pub kind: String,
    pub count: usize,
    pub added: usize,
    pub removed: usize,
    pub unchanged: usize,
    pub relationships: Vec<EdgeChange>,
}

#[derive(Clone, Debug, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct GraphView {
    pub id: String,
    pub snapshot: String,
    pub comparison: Option<String>,
    pub scope: String,
    pub breadcrumbs: Vec<Module>,
    pub nodes: Vec<ViewNode>,
    pub edges: Vec<ViewEdge>,
    pub layout_hash: String,
    pub truncated: bool,
    pub omitted_nodes: usize,
    pub omitted_edges: usize,
}

#[derive(Clone, Debug, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct TraverseRequest {
    pub snapshot: String,
    pub start: String,
    pub direction: Option<String>,
    pub depth: Option<usize>,
    pub limit: Option<usize>,
    #[serde(default)]
    pub edge_types: Vec<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct PathRequest {
    pub snapshot: String,
    pub from: String,
    pub to: String,
    #[serde(default)]
    pub edge_types: Vec<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct SourceResponse {
    pub snapshot: String,
    pub path: String,
    pub blob: String,
    pub language: String,
    pub text: String,
    pub anchor: Option<SourceRef>,
}

#[derive(Clone, Debug, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct SourceDiff {
    pub comparison: String,
    pub before_name: Option<String>,
    pub after_name: Option<String>,
    pub before: Option<SourceResponse>,
    pub after: Option<SourceResponse>,
    pub unified: String,
    pub flags: Vec<String>,
}
