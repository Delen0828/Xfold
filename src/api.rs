use crate::{jobs::Coordinator, model::*, query, repo};
use anyhow::{Context, Result};
use axum::{
    extract::{Path, Query, State},
    http::{header, HeaderMap, StatusCode, Uri},
    middleware::{self, Next},
    response::{
        sse::{Event, KeepAlive, Sse},
        IntoResponse, Response,
    },
    routing::{get, post},
    Json, Router,
};
use include_dir::{include_dir, Dir};
use serde::Deserialize;
use serde_json::{json, Value};
use std::{
    collections::BTreeMap,
    convert::Infallible,
    sync::{Arc, Mutex},
};
use tokio_stream::wrappers::ReceiverStream;
use utoipa::OpenApi;

static WEB: Dir<'_> = include_dir!("$CARGO_MANIFEST_DIR/web/dist");
pub struct AppState {
    pub coordinator: Arc<Coordinator>,
    pub initial: ReadyOrJob,
    pub default_revision: String,
    pub host: String,
    pub profile: Profile,
    pub compare_range: bool,
    snapshots: Mutex<BTreeMap<String, Arc<Snapshot>>>,
    views: Mutex<BTreeMap<String, GraphView>>,
    projections: Mutex<BTreeMap<String, Arc<query::ProjectionIndex>>>,
    comparisons: Mutex<BTreeMap<String, Arc<Comparison>>>,
}
impl AppState {
    pub fn new(
        coordinator: Arc<Coordinator>,
        initial: ReadyOrJob,
        default_revision: String,
        host: String,
        profile: Profile,
        compare_range: bool,
    ) -> Arc<Self> {
        Arc::new(Self {
            coordinator,
            initial,
            default_revision,
            host,
            profile,
            compare_range,
            comparisons: Mutex::new(BTreeMap::new()),
            snapshots: Mutex::new(BTreeMap::new()),
            views: Mutex::new(BTreeMap::new()),
            projections: Mutex::new(BTreeMap::new()),
        })
    }
    pub fn comparison(&self, id: &str) -> Result<Arc<Comparison>> {
        if let Some(c) = self.comparisons.lock().unwrap().get(id) {
            return Ok(c.clone());
        }
        let c = Arc::new(self.coordinator.store.comparison(id)?);
        let mut cache = self.comparisons.lock().unwrap();
        if cache.len() >= 4 {
            if let Some(key) = cache.keys().next().cloned() {
                cache.remove(&key);
            }
        }
        cache.insert(id.into(), c.clone());
        Ok(c)
    }
    pub fn snapshot(&self, id: &str) -> Result<Arc<Snapshot>> {
        if let Some(s) = self.snapshots.lock().unwrap().get(id) {
            return Ok(s.clone());
        }
        let s = Arc::new(self.coordinator.store.snapshot(id)?);
        let mut cache = self.snapshots.lock().unwrap();
        if cache.len() >= 4 {
            if let Some(key) = cache.keys().next().cloned() {
                cache.remove(&key);
            }
        }
        cache.insert(id.into(), s.clone());
        Ok(s)
    }
}
pub struct ApiError(anyhow::Error);
impl<E: Into<anyhow::Error>> From<E> for ApiError {
    fn from(e: E) -> Self {
        Self(e.into())
    }
}
impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        (
            StatusCode::BAD_REQUEST,
            Json(json!({"error":format!("{:#}",self.0)})),
        )
            .into_response()
    }
}
type ApiResult<T> = std::result::Result<Json<T>, ApiError>;

pub fn router(state: Arc<AppState>) -> Router {
    Router::new()
        .route("/api/v1/workspace", get(workspace))
        .route("/api/v1/refs", get(refs))
        .route("/api/v1/commits", get(commits))
        .route("/api/v1/snapshots", post(create_snapshot))
        .route("/api/v1/snapshots/{id}", get(snapshot))
        .route("/api/v1/snapshots/{id}/children", get(children))
        .route("/api/v1/snapshots/{id}/modules/{module}", get(module))
        .route("/api/v1/snapshots/{id}/search", get(search))
        .route("/api/v1/snapshots/{id}/source", get(source))
        .route("/api/v1/snapshots/{id}/modules-at", get(modules_at))
        .route("/api/v1/snapshots/{id}/diagnostics", get(diagnostics))
        .route("/api/v1/snapshots/{id}/receipt", get(receipt))
        .route("/api/v1/jobs/{id}", get(job))
        .route("/api/v1/jobs/{id}/events", get(events))
        .route("/api/v1/jobs/{id}/cancel", post(cancel))
        .route("/api/v1/graph-views", post(graph_view))
        .route("/api/v1/graph-views/{id}/edges/{edge}", get(edge))
        .route("/api/v1/traversals", post(traversal))
        .route("/api/v1/paths", post(path))
        .route("/api/v1/comparisons", post(create_comparison))
        .route("/api/v1/comparisons/{id}", get(comparison))
        .route("/api/v1/comparisons/{id}/changes", get(changes))
        .route("/api/v1/comparisons/{id}/source-diff", get(source_diff))
        .route("/api/v1/openapi.json", get(|| async { Json(openapi()) }))
        .fallback(assets)
        .layer(axum::extract::DefaultBodyLimit::max(1024 * 1024))
        .layer(middleware::from_fn_with_state(state.clone(), local_only))
        .with_state(state)
}
async fn local_only(
    State(s): State<Arc<AppState>>,
    req: axum::extract::Request,
    next: Next,
) -> Response {
    let host = req
        .headers()
        .get(header::HOST)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    let origin = req
        .headers()
        .get(header::ORIGIN)
        .and_then(|v| v.to_str().ok());
    if host != s.host || origin.is_some_and(|o| o != format!("http://{}", s.host)) {
        return (
            StatusCode::FORBIDDEN,
            Json(
                json!({"error":"Xfold accepts same-origin requests on its loopback address only"}),
            ),
        )
            .into_response();
    }
    let mut response = next.run(req).await;
    let h = response.headers_mut();
    h.insert(header::X_CONTENT_TYPE_OPTIONS, "nosniff".parse().unwrap());
    h.insert(header::REFERRER_POLICY, "no-referrer".parse().unwrap());
    h.insert(header::CONTENT_SECURITY_POLICY,"default-src 'self'; script-src 'self'; style-src 'self' 'unsafe-inline'; worker-src 'self' blob:; img-src 'self' data:; font-src 'self' data:; frame-ancestors 'none'; base-uri 'none'".parse().unwrap());
    response
}
async fn assets(uri: Uri) -> Response {
    let path = uri.path().trim_start_matches('/');
    if path.starts_with("api/") {
        return (
            StatusCode::NOT_FOUND,
            Json(json!({"error":"Unknown API route"})),
        )
            .into_response();
    }
    let file = WEB.get_file(if path.is_empty() { "index.html" } else { path });
    if let Some(f) = file {
        let mime = mime_guess::from_path(f.path())
            .first_or_octet_stream()
            .to_string();
        (
            [
                (header::CONTENT_TYPE, mime),
                (
                    header::CACHE_CONTROL,
                    if path == "index.html" || path.is_empty() {
                        "no-cache".into()
                    } else {
                        "public, max-age=31536000, immutable".into()
                    },
                ),
            ],
            f.contents(),
        )
            .into_response()
    } else {
        StatusCode::NOT_FOUND.into_response()
    }
}
async fn workspace(State(s): State<Arc<AppState>>) -> ApiResult<Value> {
    let git = repo::open(&s.coordinator.repository)?;
    let registry = sem_core::parser::plugins::create_default_registry();
    let mut extensions = registry.registered_extensions();
    extensions.sort();
    Ok(Json(
        json!({"repository":repo::identity(&git),"name":git.workdir().unwrap_or(git.path()).file_name().unwrap_or_default().to_string_lossy().trim_end_matches(".git"),"path":s.coordinator.repository,"engine":ENGINE,"initial":s.initial,"defaultRevision":s.default_revision,"compareRange":s.compare_range,"defaultProfile":s.profile,"snapshots":s.coordinator.store.list()?,"capabilities":{"extensions":extensions,"edgeTypes":["calls","typeref","imports"],"callSitePrecision":"unavailable","revisions":"commits-only","editable":false}}),
    ))
}
#[derive(Deserialize, Default)]
struct Params {
    revision: Option<String>,
    offset: Option<usize>,
    limit: Option<usize>,
    parent: Option<String>,
    q: Option<String>,
    kind: Option<String>,
    language: Option<String>,
    path: Option<String>,
    module: Option<String>,
    line: Option<usize>,
    byte: Option<usize>,
    status: Option<String>,
}
impl Params {
    fn offset(&self) -> usize {
        self.offset.unwrap_or(0)
    }
    fn limit(&self) -> usize {
        self.limit.unwrap_or(100).clamp(1, 1000)
    }
}
async fn refs(State(s): State<Arc<AppState>>) -> ApiResult<Value> {
    Ok(Json(
        json!({"items":repo::refs(&repo::open(&s.coordinator.repository)?)?}),
    ))
}
async fn commits(State(s): State<Arc<AppState>>, Query(p): Query<Params>) -> ApiResult<Value> {
    let items = repo::commits(
        &repo::open(&s.coordinator.repository)?,
        p.revision.as_deref().unwrap_or(&s.default_revision),
        p.offset(),
        p.limit(),
    )?;
    let next = if items.len() == p.limit() {
        Some(p.offset() + items.len())
    } else {
        None
    };
    Ok(Json(json!({"items":items,"nextOffset":next})))
}
async fn create_snapshot(
    State(s): State<Arc<AppState>>,
    Json(r): Json<BuildRequest>,
) -> ApiResult<ReadyOrJob> {
    Ok(Json(s.coordinator.snapshot(r).await?))
}
async fn create_comparison(
    State(s): State<Arc<AppState>>,
    Json(r): Json<CompareRequest>,
) -> ApiResult<ReadyOrJob> {
    Ok(Json(s.coordinator.comparison(r).await?))
}
async fn snapshot(State(s): State<Arc<AppState>>, Path(id): Path<String>) -> ApiResult<Value> {
    let x = s.snapshot(&id)?;
    let mut kinds: BTreeMap<String, usize> = BTreeMap::new();
    for m in x.modules.values() {
        *kinds.entry(m.kind.clone()).or_default() += 1;
    }
    Ok(Json(
        json!({"id":x.id,"commit":x.commit,"tree":x.tree,"profile":x.profile,"engine":x.engine,"dependencyFingerprint":x.dependency_fingerprint,"graphHash":x.graph_hash,"topologyHash":x.topology_hash,"moduleCount":x.modules.len(),"edgeCount":x.edges.len(),"fileCount":x.files.len(),"analyzedFiles":x.files.values().filter(|f|f.status=="analyzed").count(),"kinds":kinds,"diagnosticCount":x.diagnostics.len()}),
    ))
}
async fn children(
    State(s): State<Arc<AppState>>,
    Path(id): Path<String>,
    Query(p): Query<Params>,
) -> ApiResult<Value> {
    let snapshot = s.snapshot(&id)?;
    let parent = p.parent.as_deref().unwrap_or(ROOT);
    let mut descendants: BTreeMap<String, usize> = BTreeMap::new();
    let mut children: BTreeMap<String, usize> = BTreeMap::new();
    for module in snapshot.modules.values() {
        if let Some(parent) = &module.parent {
            *children.entry(parent.clone()).or_default() += 1;
        }
        for ancestor in query::ancestors(&snapshot.modules, &module.id) {
            *descendants.entry(ancestor).or_default() += 1;
        }
    }
    let all: Vec<_> = snapshot
        .modules
        .values()
        .filter(|m| m.parent.as_deref() == Some(parent))
        .collect();
    let items:Vec<_>=all.iter().skip(p.offset()).take(p.limit()).map(|m|{
        json!({"module":m,"childCount":children.get(&m.id).copied().unwrap_or(0),"descendantCount":descendants.get(&m.id).copied().unwrap_or(0),"dependencies":snapshot.edges.iter().filter(|e|e.from==m.id).count(),"dependents":snapshot.edges.iter().filter(|e|e.to==m.id).count()})
    }).collect();
    Ok(Json(
        json!({"snapshot":id,"items":items,"total":all.len(),"offset":p.offset()}),
    ))
}
async fn module(
    State(s): State<Arc<AppState>>,
    Path((id, module)): Path<(String, String)>,
) -> ApiResult<Value> {
    let snapshot = s.snapshot(&id)?;
    let m = snapshot.modules.get(&module).context("Unknown module")?;
    Ok(Json(
        json!({"snapshot":id,"module":m,"ancestors":query::ancestors(&snapshot.modules,&module).into_iter().map(|id|snapshot.modules[&id].clone()).collect::<Vec<_>>(),"dependencies":snapshot.edges.iter().filter(|e|e.from==module).count(),"dependents":snapshot.edges.iter().filter(|e|e.to==module).count(),"callSitePrecision":"unavailable"}),
    ))
}
async fn search(
    State(s): State<Arc<AppState>>,
    Path(id): Path<String>,
    Query(p): Query<Params>,
) -> ApiResult<Value> {
    let snapshot = s.snapshot(&id)?;
    let (items, total) = s.coordinator.store.search(
        &snapshot,
        p.q.as_deref().unwrap_or(""),
        p.kind.as_deref(),
        p.language.as_deref(),
        p.path.as_deref(),
        (p.offset(), p.limit()),
    )?;
    Ok(Json(
        json!({"snapshot":id,"items":items,"total":total,"offset":p.offset()}),
    ))
}
async fn source(
    State(s): State<Arc<AppState>>,
    Path(id): Path<String>,
    Query(p): Query<Params>,
) -> ApiResult<SourceResponse> {
    Ok(Json(query::source(
        s.snapshot(&id)?.as_ref(),
        &s.coordinator.store,
        p.path.as_deref().unwrap_or(""),
        p.module.as_deref(),
    )?))
}
async fn modules_at(
    State(s): State<Arc<AppState>>,
    Path(id): Path<String>,
    Query(p): Query<Params>,
) -> ApiResult<Value> {
    let snapshot = s.snapshot(&id)?;
    let path = p.path.context("path is required")?;
    let line = p.line.unwrap_or(1);
    let mut items: Vec<_> = snapshot
        .modules
        .values()
        .filter(|m| {
            m.is_entity()
                && m.path == path
                && m.source.as_ref().is_some_and(|s| {
                    if let Some(byte) = p.byte {
                        s.start_byte
                            .zip(s.end_byte)
                            .is_some_and(|(a, b)| a <= byte && byte < b)
                    } else {
                        s.start_line <= line && line <= s.end_line
                    }
                })
        })
        .cloned()
        .collect();
    items.sort_by_key(|m| {
        let s = m.source.as_ref().unwrap();
        (
            s.end_line - s.start_line,
            s.end_byte
                .unwrap_or(usize::MAX)
                .saturating_sub(s.start_byte.unwrap_or(0)),
            m.id.clone(),
        )
    });
    Ok(Json(json!({"snapshot":id,"items":items})))
}
async fn diagnostics(
    State(s): State<Arc<AppState>>,
    Path(id): Path<String>,
    Query(p): Query<Params>,
) -> ApiResult<Value> {
    let x = s.snapshot(&id)?;
    Ok(Json(
        json!({"snapshot":id,"total":x.diagnostics.len(),"items":x.diagnostics.iter().skip(p.offset()).take(p.limit()).collect::<Vec<_>>()}),
    ))
}
async fn receipt(
    State(s): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> ApiResult<BuildReceipt> {
    Ok(Json(s.coordinator.store.receipt(&id)?))
}
async fn job(State(s): State<Arc<AppState>>, Path(id): Path<String>) -> ApiResult<Job> {
    Ok(Json(s.coordinator.job(&id)?))
}
async fn cancel(State(s): State<Arc<AppState>>, Path(id): Path<String>) -> ApiResult<Job> {
    Ok(Json(s.coordinator.cancel(&id)?))
}
async fn events(
    State(s): State<Arc<AppState>>,
    Path(id): Path<String>,
    _headers: HeaderMap,
) -> std::result::Result<Response, ApiError> {
    let mut receiver = s.coordinator.subscribe();
    let initial = s.coordinator.job(&id)?;
    let (tx, rx) = tokio::sync::mpsc::channel(16);
    tokio::spawn(async move {
        let send = |j: &Job| {
            Ok::<Event, Infallible>(
                Event::default()
                    .id(j.sequence.to_string())
                    .event("job")
                    .json_data(j)
                    .unwrap(),
            )
        };
        let mut last = initial.sequence;
        if tx.send(send(&initial)).await.is_err()
            || ["ready", "failed", "cancelled"].contains(&initial.state.as_str())
        {
            return;
        }
        loop {
            let job = match receiver.recv().await {
                Ok(j) => j,
                Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => {
                    match s.coordinator.job(&id) {
                        Ok(j) => j,
                        Err(_) => break,
                    }
                }
                Err(_) => break,
            };
            if job.id != id || job.sequence <= last {
                continue;
            }
            last = job.sequence;
            if tx.send(send(&job)).await.is_err()
                || ["ready", "failed", "cancelled"].contains(&job.state.as_str())
            {
                break;
            }
        }
    });
    Ok(Sse::new(ReceiverStream::new(rx))
        .keep_alive(KeepAlive::default())
        .into_response())
}
async fn graph_view(
    State(s): State<Arc<AppState>>,
    Json(r): Json<GraphRequest>,
) -> ApiResult<GraphView> {
    if r.snapshot.is_some() == r.comparison.is_some() {
        return Err(anyhow::anyhow!("Specify exactly one snapshot or comparison").into());
    }
    let c = r
        .comparison
        .as_ref()
        .map(|id| s.comparison(id))
        .transpose()?;
    let target = s.snapshot(
        c.as_ref()
            .map(|c| c.target.as_str())
            .or(r.snapshot.as_deref())
            .unwrap(),
    )?;
    let base = c.as_ref().map(|c| s.snapshot(&c.base)).transpose()?;
    let key = crate::store::digest(&(&r, &target.id, c.as_ref().map(|c| &c.id)))?;
    if let Some(view) = s.views.lock().unwrap().get(&key) {
        let mut view = view.clone();
        for edge in &mut view.edges {
            edge.relationships.clear();
        }
        return Ok(Json(view));
    }
    let artifact = c.as_ref().map(|c| c.id.as_str()).unwrap_or(&target.id);
    let index = { s.projections.lock().unwrap().get(artifact).cloned() };
    let index = if let Some(index) = index {
        index
    } else {
        let index = Arc::new(query::ProjectionIndex::new(
            &target,
            base.as_deref(),
            c.as_deref(),
        ));
        let mut cache = s.projections.lock().unwrap();
        if cache.len() >= 2 {
            if let Some(key) = cache.keys().next().cloned() {
                cache.remove(&key);
            }
        }
        cache.insert(artifact.into(), index.clone());
        index
    };
    let mut view = query::graph_view_indexed(&target, base.as_deref(), c.as_deref(), &r, &index)?;
    {
        let mut views = s.views.lock().unwrap();
        if views.len() >= 32 {
            if let Some(id) = views.keys().next().cloned() {
                views.remove(&id);
            }
        }
        views.insert(view.id.clone(), view.clone());
    }
    for edge in &mut view.edges {
        edge.relationships.clear();
    }
    Ok(Json(view))
}
async fn edge(
    State(s): State<Arc<AppState>>,
    Path((id, edge)): Path<(String, String)>,
    Query(p): Query<Params>,
) -> ApiResult<Value> {
    let views = s.views.lock().unwrap();
    let view = views
        .get(&id)
        .context("Graph view expired; reload the view")?;
    let edge = view
        .edges
        .iter()
        .find(|e| e.id == edge)
        .context("Unknown aggregate edge")?;
    let comparison = view
        .comparison
        .as_ref()
        .map(|id| s.comparison(id))
        .transpose()?;
    let items=edge.relationships.iter().skip(p.offset()).take(p.limit()).map(|relation|->Result<Value>{
        let snapshot_id=if relation.side=="base"{comparison.as_ref().map(|c|c.base.as_str()).unwrap_or(&view.snapshot)}else{&view.snapshot};
        let snapshot=s.snapshot(snapshot_id)?;
        let mut value=serde_json::to_value(relation)?;
        value["source"]=json!({"snapshot":snapshot_id,"module":relation.edge.from,"anchor":snapshot.modules.get(&relation.edge.from).and_then(|m|m.source.as_ref())});
        value["target"]=json!({"snapshot":snapshot_id,"module":relation.edge.to,"anchor":snapshot.modules.get(&relation.edge.to).and_then(|m|m.source.as_ref())});
        Ok(value)
    }).collect::<Result<Vec<_>>>()?;
    Ok(Json(
        json!({"snapshot":view.snapshot,"comparison":view.comparison,"view":id,"edge":edge.id,"total":edge.count,"items":items,"callSitePrecision":"unavailable"}),
    ))
}
async fn traversal(
    State(s): State<Arc<AppState>>,
    Json(r): Json<TraverseRequest>,
) -> ApiResult<Value> {
    Ok(Json(query::traversal(
        s.snapshot(&r.snapshot)?.as_ref(),
        &r,
    )?))
}
async fn path(State(s): State<Arc<AppState>>, Json(r): Json<PathRequest>) -> ApiResult<Value> {
    Ok(Json(query::shortest_path(
        s.snapshot(&r.snapshot)?.as_ref(),
        &r,
    )?))
}
async fn comparison(
    State(s): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> ApiResult<Comparison> {
    Ok(Json((*s.comparison(&id)?).clone()))
}
async fn changes(
    State(s): State<Arc<AppState>>,
    Path(id): Path<String>,
    Query(p): Query<Params>,
) -> ApiResult<Value> {
    let c = s.comparison(&id)?;
    let target = s.snapshot(&c.target)?;
    let base = s.snapshot(&c.base)?;
    let all: Vec<_> = query::change_seeds(&c, &target, &base)
        .into_iter()
        .filter(|c| p.status.as_ref().is_none_or(|f| c.flags.contains(f)))
        .collect();
    let items: Vec<_> = all
        .iter()
        .skip(p.offset())
        .take(p.limit())
        .map(|change| {
            let module = change
                .after
                .as_ref()
                .and_then(|id| target.modules.get(id))
                .or_else(|| change.before.as_ref().and_then(|id| base.modules.get(id)));
            let mut value = json!(change);
            value["module"] = json!(module);
            value
        })
        .collect();
    Ok(Json(
        json!({"comparison":id,"summary":c.summary,"items":items,"total":all.len()}),
    ))
}
async fn source_diff(
    State(s): State<Arc<AppState>>,
    Path(id): Path<String>,
    Query(p): Query<Params>,
) -> ApiResult<SourceDiff> {
    let c = s.comparison(&id)?;
    Ok(Json(query::source_diff(
        &c,
        s.snapshot(&c.base)?.as_ref(),
        s.snapshot(&c.target)?.as_ref(),
        &s.coordinator.store,
        p.module.as_deref(),
        p.path.as_deref(),
    )?))
}

#[derive(OpenApi)]
#[openapi(
    info(title = "Xfold local API", version = "1.0.0"),
    components(schemas(
        Profile,
        SourceRef,
        Module,
        Dependency,
        Diagnostic,
        FileRecord,
        Snapshot,
        BuildStats,
        BuildReceipt,
        Change,
        EdgeChange,
        Comparison,
        BuildRequest,
        CompareRequest,
        Job,
        ReadyOrJob,
        GraphRequest,
        ViewNode,
        ViewEdge,
        GraphView,
        TraverseRequest,
        PathRequest,
        SourceResponse,
        SourceDiff
    ))
)]
struct Contract;

pub fn openapi() -> Value {
    let mut doc = serde_json::to_value(Contract::openapi()).unwrap();
    let mut paths = serde_json::Map::new();
    // Endpoint metadata references schemas generated directly from Rust DTOs.
    let endpoints = [
        ("/workspace", "get", "workspace", None, None),
        ("/refs", "get", "refs", None, None),
        ("/commits", "get", "commits", None, None),
        (
            "/snapshots",
            "post",
            "createSnapshot",
            Some("BuildRequest"),
            Some("ReadyOrJob"),
        ),
        ("/snapshots/{id}", "get", "snapshot", None, None),
        ("/snapshots/{id}/children", "get", "children", None, None),
        (
            "/snapshots/{id}/modules/{module}",
            "get",
            "module",
            None,
            None,
        ),
        ("/snapshots/{id}/search", "get", "search", None, None),
        (
            "/snapshots/{id}/source",
            "get",
            "source",
            None,
            Some("SourceResponse"),
        ),
        ("/snapshots/{id}/modules-at", "get", "modulesAt", None, None),
        (
            "/snapshots/{id}/diagnostics",
            "get",
            "diagnostics",
            None,
            None,
        ),
        (
            "/snapshots/{id}/receipt",
            "get",
            "receipt",
            None,
            Some("BuildReceipt"),
        ),
        ("/jobs/{id}", "get", "job", None, Some("Job")),
        ("/jobs/{id}/events", "get", "events", None, None),
        ("/jobs/{id}/cancel", "post", "cancel", None, Some("Job")),
        (
            "/graph-views",
            "post",
            "graphView",
            Some("GraphRequest"),
            Some("GraphView"),
        ),
        ("/graph-views/{id}/edges/{edge}", "get", "edge", None, None),
        (
            "/traversals",
            "post",
            "traversal",
            Some("TraverseRequest"),
            None,
        ),
        ("/paths", "post", "path", Some("PathRequest"), None),
        (
            "/comparisons",
            "post",
            "createComparison",
            Some("CompareRequest"),
            Some("ReadyOrJob"),
        ),
        (
            "/comparisons/{id}",
            "get",
            "comparison",
            None,
            Some("Comparison"),
        ),
        ("/comparisons/{id}/changes", "get", "changes", None, None),
        (
            "/comparisons/{id}/source-diff",
            "get",
            "sourceDiff",
            None,
            Some("SourceDiff"),
        ),
    ];
    for (path, method, operation, input, output) in endpoints {
        let response = output
            .map(|name| json!({"$ref":format!("#/components/schemas/{name}")}))
            .unwrap_or(json!({"type":"object","additionalProperties":true}));
        let mut op = json!({"operationId":operation,"responses":{"200":{"description":"Success","content":{"application/json":{"schema":response}}},"400":{"description":"Invalid request or unavailable artifact"}}});
        let mut params = Vec::new();
        for part in path.split('/').filter(|p| p.starts_with('{')) {
            params.push(json!({"name":part.trim_matches(['{','}']),"in":"path","required":true,"schema":{"type":"string"}}));
        }
        if method == "get" {
            for name in [
                "revision", "parent", "q", "kind", "language", "path", "module", "status",
            ] {
                params.push(
                    json!({"name":name,"in":"query","required":false,"schema":{"type":"string"}}),
                );
            }
            for name in ["offset", "limit", "line", "byte"] {
                params.push(json!({"name":name,"in":"query","required":false,"schema":{"type":"integer","minimum":0}}));
            }
        }
        op["parameters"] = json!(params);
        if let Some(input) = input {
            op["requestBody"] = json!({"required":true,"content":{"application/json":{"schema":{"$ref":format!("#/components/schemas/{input}")}}}});
        }
        if operation == "events" {
            op["responses"]["200"]["content"] =
                json!({"text/event-stream":{"schema":{"type":"string"}}});
        }
        paths.insert(format!("/api/v1{path}"), json!({method:op}));
    }
    doc["paths"] = Value::Object(paths);
    doc
}
