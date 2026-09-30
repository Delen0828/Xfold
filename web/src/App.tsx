import { memo, useCallback, useEffect, useRef, useState } from 'react';
import { Background, BackgroundVariant, Controls, Handle, MiniMap, Position, ReactFlow, useReactFlow, type Node, type NodeProps, type Edge, type Viewport } from '@xyflow/react';
import { api, params } from './client';
import { createSnapshot, createComparison } from './generated-client';
import { CodePane } from './CodePane';
import { palette, primaryFlag } from './palette';
import type { BuildReceipt, Commit, Comparison, EdgeChange, GraphView, Job, Module, ReadyOrJob, SnapshotInfo, SourceDiff, SourceResponse, ViewNode, Workspace } from './types';

type ModuleData = ViewNode & { onEnter: (node: ViewNode) => void; onToggle: (id: string) => void; expanded: boolean; faded: boolean };
type ModuleNode = Node<ModuleData, 'module'>;
const SemanticNode = memo(({ data, selected }: NodeProps<ModuleNode>) => {
  const flag = primaryFlag(data.flags); const color = flag ? palette[flag].color : undefined;
  return <div className={`semantic-node ${selected ? 'is-selected' : ''} ${flag || ''} ${data.boundary ? 'boundary' : ''} ${data.faded ? 'faded' : ''}`} style={{ '--status-color': color } as React.CSSProperties} title={data.qualifiedName || data.path}>
    <Handle type="target" position={Position.Left}/><Handle type="source" position={Position.Right}/>
    <div className="node-top"><span className={`node-icon ${data.semId ? 'entity' : ''}`}>{data.kind === 'directory' ? '▱' : data.kind === 'file' ? '≡' : data.kind === 'class' ? 'C' : 'ƒ'}</span><span className="node-kind">{data.kind}</span>{data.boundary && <span className="node-outside">↗</span>}{flag && <span className="node-status" title={data.flags.join(', ')}>{palette[flag].symbol} {palette[flag].label}</span>}</div>
    <div className="node-name">{data.name}</div>
    <div className="node-bottom"><span>{data.childCount ? `${data.childCount} children` : data.source ? `L${data.source.startLine}–${data.source.endLine}` : data.path.split('/').slice(0, -1).join('/') || 'container'}{data.changedDescendants > 0 && <b> · {data.changedDescendants} changed</b>}{data.movedOut > 0 && <b> · {data.movedOut} moved out</b>}</span>{data.childCount > 0 && <span className="node-actions"><button className="nodrag" aria-label={`${data.expanded ? 'Collapse' : 'Expand'} ${data.name}`} onClick={e => { e.stopPropagation(); data.onToggle(data.id); }}>{data.expanded ? '−' : '+'}</button><button className="nodrag" aria-label={`Enter ${data.name}`} onClick={e => { e.stopPropagation(); data.onEnter(data); }}>↗</button></span>}</div>
  </div>;
});
const nodeTypes = { module: SemanticNode };
const empty = <span>—</span>;

export function App() {
  const flow = useReactFlow<ModuleNode>();
  const [workspace, setWorkspace] = useState<Workspace | null>(null);
  const [snapshot, setSnapshot] = useState<SnapshotInfo | null>(null);
  const [comparison, setComparison] = useState<Comparison | null>(null);
  const [view, setView] = useState<GraphView | null>(null);
  const [nodes, setNodes] = useState<ModuleNode[]>([]);
  const [edges, setEdges] = useState<Edge[]>([]);
  const [scope, setScope] = useState('root');
  const [expanded, setExpanded] = useState<string[]>([]);
  const [selected, setSelected] = useState<ViewNode | null>(null);
  const [source, setSource] = useState<SourceResponse | null>(null);
  const [diff, setDiff] = useState<SourceDiff | null>(null);
  const [sourceLoading, setSourceLoading] = useState(false);
  const [job, setJob] = useState<Job | null>(null);
  const [pendingJob, setPendingJob] = useState<string | null>(null);
  const [error, setError] = useState('');
  const [commits, setCommits] = useState<Commit[]>([]);
  const [refs, setRefs] = useState<{ name: string; commit: string }[]>([]);
  const [historyOffset, setHistoryOffset] = useState<number | null>(null);
  const [historyOpen, setHistoryOpen] = useState(false);
  const [targetRef, setTargetRef] = useState('HEAD');
  const [baseRef, setBaseRef] = useState('');
  const [baseCommit, setBaseCommit] = useState('');
  const [compareEnabled, setCompareEnabled] = useState(false);
  const [kind, setKind] = useState('');
  const [edgeType, setEdgeType] = useState('');
  const [changeFilter, setChangeFilter] = useState('');
  const [searchText, setSearchText] = useState('');
  const [searchResults, setSearchResults] = useState<Module[]>([]);
  const [searchOpen, setSearchOpen] = useState(false);
  const [showMap, setShowMap] = useState(false);
  const [guide, setGuide] = useState(false);
  const [theme, setTheme] = useState(() => localStorage.getItem('xfold.theme') || 'dark');
  const [ratio, setRatio] = useState(() => Number(localStorage.getItem('xfold.split') || 66.67));
  const [reducedMotion, setReducedMotion] = useState(matchMedia('(prefers-reduced-motion: reduce)').matches);
  const [hidden, setHidden] = useState(document.hidden);
  const [layoutBusy, setLayoutBusy] = useState(false);
  const [inspector, setInspector] = useState<{ title: string; modules?: (Module & { snapshotSide?: 'base' | 'target' })[]; relations?: EdgeChange[]; total?: number; detail?: string; side?: string } | null>(null);
  const [traversalSide, setTraversalSide] = useState<'base' | 'target'>('target');
  const [routePicking, setRoutePicking] = useState(false);
  const [highlightIds, setHighlightIds] = useState<Set<string>>(new Set());
  const [receipt, setReceipt] = useState<BuildReceipt | null>(null);
  const searchInput = useRef<HTMLInputElement>(null);
  const loadSequence = useRef(0);
  const graphSequence = useRef(0);
  const worker = useRef<Worker | null>(null);
  const positions = useRef<Record<string, { x: number; y: number }>>({});
  const reveal = useRef<string | null>(null);
  const pendingScope = useRef<string | null>(null);
  const initialized = useRef(false);
  const currentView = useRef<GraphView | null>(null);
  const exitTimer = useRef<ReturnType<typeof setTimeout> | null>(null);
  const selectedId = useRef(selected?.id); selectedId.current = selected?.id;
  const motionRef = useRef(false); motionRef.current = !reducedMotion && !hidden;
  const motion = !reducedMotion && !hidden;

  const showError = useCallback((e: unknown) => { if (!(e instanceof DOMException && e.name === 'AbortError')) setError(e instanceof Error ? e.message : String(e)); }, []);
  const loadArtifact = useCallback(async (id: string, artifactKind: string) => {
    const sequence = ++loadSequence.current;
    try {
      const c = artifactKind === 'comparison' ? await api<Comparison>(`/comparisons/${id}`) : null;
      const [next, base] = await Promise.all([api<SnapshotInfo>(`/snapshots/${c?.target || id}`), c ? api<SnapshotInfo>(`/snapshots/${c.base}`) : Promise.resolve(null)]);
      if (sequence !== loadSequence.current) return;
      setComparison(c); setCompareEnabled(!!c); setSnapshot(next); setSelected(null); setInspector(null); setHighlightIds(new Set()); setExpanded([]); setScope(pendingScope.current || 'root'); pendingScope.current = null;
      setPendingJob(null); setJob(null); setTargetRef(next.commit);
      setBaseCommit(base?.commit || ''); if (base) setBaseRef(base.commit);
      api<BuildReceipt>(`/snapshots/${next.id}/receipt`).then(r => { if (sequence === loadSequence.current) setReceipt(r); }).catch(showError);
    } catch (e) { showError(e); }
  }, [showError]);
  const accept = useCallback((result: ReadyOrJob) => { if (result.state === 'ready') void loadArtifact(result.id, result.kind); else { setPendingJob(result.id); setJob(null); } }, [loadArtifact]);

  useEffect(() => {
    if (initialized.current) return; initialized.current = true;
    api<Workspace>('/workspace').then(async w => {
      setWorkspace(w);
      api<{ items: { name: string; commit: string }[] }>('/refs').then(r => setRefs(r.items)).catch(showError);
      const hash = new URLSearchParams(location.hash.slice(1));
      reveal.current = hash.get('node'); pendingScope.current = hash.get('scope');
      const target = hash.get('commit'); const base = hash.get('base');
      if (target) { setTargetRef(target); if (base) { setBaseRef(base); accept(await api<ReadyOrJob>('/comparisons', { base, target, profile: w.defaultProfile })); } else accept(await api<ReadyOrJob>('/snapshots', { revision: target, profile: w.defaultProfile })); }
      else accept(w.initial);
      const history = await api<{ items: Commit[]; nextOffset: number | null }>(`/commits?${params({ revision: w.defaultRevision, limit: 30 })}`); setCommits(history.items); setHistoryOffset(history.nextOffset);
    }).catch(showError);
  }, [accept, showError]);
  useEffect(() => {
    if (!pendingJob) return;
    let done = false;
    const onJob = (j: Job) => {
      if (done) return; setJob(j);
      if (j.state === 'ready' && j.resultId) { done = true; events.close(); void loadArtifact(j.resultId, j.resultKind); }
      else if (j.state === 'failed' || j.state === 'cancelled') { done = true; events.close(); setPendingJob(null); if (j.error) setError(j.error); }
    };
    const events = new EventSource(`/api/v1/jobs/${pendingJob}/events`);
    events.addEventListener('job', e => onJob(JSON.parse((e as MessageEvent).data)));
    const timer = setInterval(() => { if (!done) api<Job>(`/jobs/${pendingJob}`).then(onJob).catch(showError); }, 1500);
    return () => { done = true; events.close(); clearInterval(timer); };
  }, [pendingJob, loadArtifact, showError]);

  useEffect(() => { document.documentElement.dataset.theme = theme; localStorage.setItem('xfold.theme', theme); }, [theme]);
  useEffect(() => {
    const mq = matchMedia('(prefers-reduced-motion: reduce)'); const change = () => setReducedMotion(mq.matches); const visibility = () => setHidden(document.hidden);
    mq.addEventListener('change', change); document.addEventListener('visibilitychange', visibility); return () => { mq.removeEventListener('change', change); document.removeEventListener('visibilitychange', visibility); };
  }, []);
  useEffect(() => {
    const key = (e: KeyboardEvent) => {
      const t = e.target as HTMLElement;
      if (t.closest('input,textarea,.monaco-editor,[contenteditable="true"]')) return;
      if (e.key === 'Enter' && e.altKey) {
        const node = currentView.current?.nodes.find(n => n.id === selectedId.current);
        if (node && node.childCount > 0) { e.preventDefault(); setScope(node.id); setExpanded([]); setInspector(null); }
      }
      if (e.key === '/') { e.preventDefault(); setSearchOpen(true); searchInput.current?.focus(); }
      if (e.key === 'Escape') { setSearchOpen(false); setInspector(null); setGuide(false); setRoutePicking(false); setHighlightIds(new Set()); }
      if (e.key === '0') void flow.fitView({ padding: 0.18, duration: motion ? 250 : 0 });
      if (e.key === '+' || e.key === '=') void flow.zoomIn({ duration: motion ? 160 : 0 });
      if (e.key === '-') void flow.zoomOut({ duration: motion ? 160 : 0 });
      if (e.key.toLowerCase() === 'm') setShowMap(v => !v);
      if (e.key === '?') setGuide(v => !v);
    }; addEventListener('keydown', key); return () => removeEventListener('keydown', key);
  }, [flow, motion]);

  const enter = useCallback((node: ViewNode) => { setScope(node.id); setExpanded([]); setInspector(null); }, []);
  const toggle = useCallback((id: string) => setExpanded(old => old.includes(id) ? old.filter(x => x !== id) : [...old, id]), []);
  const renderView = useCallback((v: GraphView, p: Record<string, { x: number; y: number }>, fit: boolean) => {
    positions.current = p; setLayoutBusy(false);
    const nextNodes: ModuleNode[] = v.nodes.map(n => ({ id: n.id, selected: n.id === selectedId.current, type: 'module', position: p[n.id] || { x: 0, y: 0 }, data: { ...n, onEnter: enter, onToggle: toggle, expanded: expanded.includes(n.id), faded: false }, ariaLabel: `${n.kind} ${n.name} ${n.flags.join(' ')}` }));
    if (exitTimer.current) clearTimeout(exitTimer.current);
    const retained = new Set(nextNodes.map(n => n.id));
    setNodes(old => [...nextNodes, ...(motionRef.current ? old.filter(n => !retained.has(n.id)).map(n => ({ ...n, selected: false, selectable: false, focusable: false, style: { opacity: 0, pointerEvents: 'none' as const } })) : [])]);
    if (motionRef.current) exitTimer.current = setTimeout(() => setNodes(ns => ns.filter(n => retained.has(n.id))), 180);
    setEdges(v.edges.map(e => ({ id: e.id, source: e.from, target: e.to, type: 'smoothstep', label: `${e.kind}${e.count > 1 ? ` ×${e.count}` : ''}${e.added ? ` +${e.added}` : ''}${e.removed ? ` −${e.removed}` : ''}`, animated: false, style: { stroke: e.removed && !e.added && !e.unchanged ? palette.deleted.color : e.added && !e.removed && !e.unchanged ? palette.added.color : e.added || e.removed ? palette.modified.color : '#526277', strokeWidth: e.added || e.removed ? 1.8 : 1.2, strokeDasharray: e.removed && !e.added && !e.unchanged ? '5 4' : undefined }, markerEnd: { type: 'arrowclosed' as never, color: e.removed && !e.added ? palette.deleted.color : '#798ca6', width: 16, height: 16 }, labelStyle: { fill: '#a2adc0', fontSize: 10 }, labelBgStyle: { fill: '#111923', fillOpacity: 0.95 }, labelBgPadding: [6, 3] })));
    if (fit) requestAnimationFrame(() => requestAnimationFrame(() => { const saved = localStorage.getItem(`xfold.camera.${workspace?.repository}.${v.scope}`); if (saved) { try { void flow.setViewport(JSON.parse(saved), { duration: 0 }); } catch { void flow.fitView({ padding: .2, duration: 0 }); } } else void flow.fitView({ padding: .2, duration: 0 }); }));
    if (reveal.current) { const node = v.nodes.find(n => n.id === reveal.current); if (node) { setSelected(node); reveal.current = null; } }
  }, [enter, toggle, expanded, flow, workspace?.repository]);

  useEffect(() => {
    if (!snapshot) return;
    const controller = new AbortController(); const sequence = ++graphSequence.current;
    setLayoutBusy(true);
    api<GraphView>('/graph-views', { snapshot: comparison ? null : snapshot.id, comparison: comparison?.id || null, scope, expanded, kinds: kind ? [kind] : [], changes: changeFilter ? [changeFilter] : [], edgeTypes: edgeType ? [edgeType] : [], limit: 300 }, controller.signal).then(v => {
      if (sequence !== graphSequence.current) return;
      const previous = currentView.current;
      const fit = !previous || previous.scope !== v.scope; currentView.current = v; setView(v);
      if (previous?.comparison && previous.comparison === v.comparison && previous.scope === v.scope && v.nodes.every(n => positions.current[n.id])) { renderView(v, positions.current, fit); return; }
      const key = `xfold.layout.${v.layoutHash}`; const cached = localStorage.getItem(key);
      if (cached) { try { renderView(v, JSON.parse(cached), fit); return; } catch { localStorage.removeItem(key); } }
      worker.current?.terminate(); const w = new Worker(new URL('./layout.worker.ts', import.meta.url), { type: 'module' }); worker.current = w;
      w.onmessage = ({ data }) => {
        if (sequence !== graphSequence.current) return;
        if (data.error) { showError(data.error); setLayoutBusy(false); return; }
        try { const keys = Object.keys(localStorage).filter(k => k.startsWith('xfold.layout.')); if (keys.length > 20) localStorage.removeItem(keys[0]); localStorage.setItem(key, JSON.stringify(data.positions)); } catch { /* layout caching is optional */ }
        renderView(v, data.positions, fit);
      };
      w.onerror = e => { showError(e.message); setLayoutBusy(false); };
      w.postMessage({ sequence, hash: v.layoutHash, nodes: v.nodes, edges: v.edges });
    }).catch(e => { if (sequence === graphSequence.current) { setLayoutBusy(false); showError(e); } });
    return () => controller.abort();
  }, [snapshot, comparison, scope, expanded, kind, changeFilter, edgeType, renderView, showError]);
  useEffect(() => () => { worker.current?.terminate(); if (exitTimer.current) clearTimeout(exitTimer.current); }, []);
  useEffect(() => { setNodes(ns => ns.map(n => ({ ...n, selected: n.id === selected?.id, data: { ...n.data, faded: highlightIds.size > 0 && !highlightIds.has(n.id) } }))); }, [selected, highlightIds]);

  useEffect(() => {
    setSource(null); setDiff(null); if (!selected?.source) { setSourceLoading(false); return; }
    const controller = new AbortController(); setSourceLoading(true);
    const task = comparison ? api<SourceDiff>(`/comparisons/${comparison.id}/source-diff?${params({ module: selected.id })}`, undefined, controller.signal).then(setDiff) : api<SourceResponse>(`/snapshots/${selected.snapshot}/source?${params({ module: selected.id })}`, undefined, controller.signal).then(setSource);
    task.catch(showError).finally(() => { if (!controller.signal.aborted) setSourceLoading(false); });
    return () => controller.abort();
  }, [selected?.id, selected?.snapshot, comparison?.id, showError]);
  useEffect(() => {
    if (!snapshot || !searchText.trim()) { setSearchResults([]); return; }
    const controller = new AbortController(); const timer = setTimeout(() => {
      api<{ items: Module[] }>(`/snapshots/${snapshot.id}/search?${params({ q: searchText, limit: 30, kind: kind || undefined })}`, undefined, controller.signal).then(r => setSearchResults(r.items)).catch(showError);
    }, 180); return () => { clearTimeout(timer); controller.abort(); };
  }, [snapshot, searchText, kind, showError]);
  useEffect(() => {
    if (!snapshot || !workspace || comparison && !baseCommit) return;
    const hash = params({ commit: snapshot.commit, base: comparison ? baseCommit : undefined, node: selected?.id, scope });
    history.replaceState(null, '', `#${hash}`);
  }, [snapshot, comparison, selected, scope, baseCommit, workspace]);

  const openModule = async (module: Module, requestedSide?: string) => {
    const side = requestedSide || (comparison?.changes.some(c => c.before === module.id && !c.after) ? 'base' : 'target');
    if (!snapshot) return;
    setSearchOpen(false); setSearchText('');
    const mapped = side === 'base' && comparison ? comparison.changes.find(c => c.before === module.id)?.id || module.id : module.id;
    const present = view?.nodes.find(n => n.id === mapped);
    if (present) { setSelected(present); const p = positions.current[present.id]; if (p) void flow.setCenter(p.x + 119, p.y + 50, { zoom: Math.max(.8, flow.getZoom()), duration: motion ? 240 : 0 }); return; }
    reveal.current = mapped;
    const parent = side === 'base' && comparison && module.parent ? comparison.changes.find(c => c.before === module.parent)?.id || module.parent : module.parent;
    setKind(''); setChangeFilter(''); setScope(parent || 'root'); setExpanded([]);
    setSelected({ ...module, id: mapped, snapshot: side === 'base' && comparison ? comparison.base : snapshot.id, side, before: side === 'base' ? module.id : null, flags: comparison?.changes.find(c => c.id === mapped)?.flags || [], childCount: 0, descendantCount: 0, changedDescendants: 0, movedOut: 0, boundary: false });
  };
  const inspectSide = () => comparison && traversalSide === 'base' ? comparison.base : snapshot!.id;
  const selectedForSide = () => traversalSide === 'base' && comparison ? selected?.before || comparison.changes.find(c => c.id === selected?.id)?.before : selected?.side === 'base' ? null : selected?.id;
  const traverse = async (direction: string) => {
    const start = selectedForSide(); if (!start) { setError(`This module does not exist in the ${traversalSide} snapshot.`); return; }
    try { const result = await api<{ nodes: Module[]; truncated: boolean }>('/traversals', { snapshot: inspectSide(), start, direction, depth: 1, limit: 300, edgeTypes: edgeType ? [edgeType] : [] }); setInspector({ title: direction === 'dependencies' ? 'Dependencies' : 'Dependents', modules: result.nodes.filter(n => n.id !== start), side: traversalSide, detail: result.truncated ? 'Results limited to 300 modules.' : `Direct relationships in the ${traversalSide} snapshot` }); setHighlightIds(new Set(result.nodes.map(n => comparison && traversalSide === 'base' ? comparison.changes.find(c => c.before === n.id)?.id || n.id : n.id))); } catch (e) { showError(e); }
  };
  const selectNode = async (node: ViewNode) => {
    if (!routePicking) { setSelected(node); return; }
    setRoutePicking(false);
    const from = selectedForSide(); const to = traversalSide === 'base' && comparison ? node.before : node.side === 'base' ? null : node.id;
    if (!from || !to) { setError('Both route endpoints must exist in the selected snapshot.'); return; }
    try { const result = await api<{ reachable: boolean; nodes: string[] }>('/paths', { snapshot: inspectSide(), from, to, edgeTypes: edgeType ? [edgeType] : [] }); const ms = await Promise.all(result.nodes.map(id => api<{ module: Module }>(`/snapshots/${inspectSide()}/modules/${id}`))); setInspector({ title: 'Directed route', modules: ms.map(x => x.module), side: traversalSide, detail: result.reachable ? `${result.nodes.length - 1} directed steps · ${traversalSide} snapshot` : 'No directed route exists in this snapshot.' }); setHighlightIds(new Set(result.nodes.map(id => comparison && traversalSide === 'base' ? comparison.changes.find(c => c.before === id)?.id || id : id))); } catch (e) { showError(e); }
  };
  const build = async () => { setError(''); try { accept(compareEnabled ? await createComparison({ base: baseRef || `${targetRef}^1`, target: targetRef, profile: snapshot?.profile || workspace?.defaultProfile }) : await createSnapshot({ revision: targetRef, profile: snapshot?.profile || workspace?.defaultProfile })); } catch (e) { showError(e); } };
  const changeList = async () => {
    if (!comparison) return;
    const changed = comparison.changes.filter(c => c.flags.some(f => f !== 'location'));
    const ms = await Promise.all(changed.slice(0, 100).map(c => api<{ module: Module }>(`/snapshots/${c.after ? comparison.target : comparison.base}/modules/${c.after || c.before}`)));
    setInspector({ title: 'Changed modules', modules: ms.map((x, i) => ({ ...x.module, snapshotSide: changed[i].after ? 'target' : 'base' })), total: changed.length, detail: `${changed.length} changed entities and containers · showing first ${ms.length}` });
  };
  const resize = (e: React.PointerEvent) => {
    e.currentTarget.setPointerCapture(e.pointerId);
    const move = (event: PointerEvent) => { const value = Math.max(35, Math.min(78, event.clientX / window.innerWidth * 100)); setRatio(value); localStorage.setItem('xfold.split', String(value)); };
    const stop = () => { removeEventListener('pointermove', move); removeEventListener('pointerup', stop); }; addEventListener('pointermove', move); addEventListener('pointerup', stop);
  };
  const onCodePosition = (value: SourceResponse, line: number, byte: number) => { api<{ items: Module[] }>(`/snapshots/${value.snapshot}/modules-at?${params({ path: value.path, line, byte })}`).then(r => r.items[0] && openModule(r.items[0], value.snapshot === comparison?.base ? 'base' : 'target')).catch(showError); };
  const saveCamera = (_: unknown, viewport: Viewport) => { if (workspace && view) localStorage.setItem(`xfold.camera.${workspace.repository}.${view.scope}`, JSON.stringify(viewport)); };

  return <div className={`app ${motion ? 'motion' : 'still'}`}>
    <header className="topbar"><a className="brand" href="#" onClick={e => { e.preventDefault(); setScope('root'); }}><svg viewBox="0 0 28 28" aria-hidden="true"><path d="M4 5 14 14 4 23M14 5 24 14 14 23"/></svg><span>Xfold</span><span className="version">/ local</span></a><div className="repo-label"><span>▱</span> {workspace?.name || 'Opening repository'}<span className="repo-local">LOCAL REPOSITORY</span></div><div className="top-actions"><button className="icon-button" title="Copy link to this view" aria-label="Copy view link" onClick={() => navigator.clipboard.writeText(location.href).catch(showError)}>↗</button><button className="icon-button" title="Toggle theme" aria-label="Toggle theme" onClick={() => setTheme(t => t === 'dark' ? 'light' : 'dark')}>{theme === 'dark' ? '☼' : '☾'}</button><button className="icon-button" aria-label="Keyboard guide" onClick={() => setGuide(v => !v)}>?</button></div></header>
    <div className="revision-bar"><div className="revision-mode"><button className={!compareEnabled ? 'active' : ''} onClick={() => setCompareEnabled(false)}>Explore</button><button className={compareEnabled ? 'active' : ''} onClick={() => { setCompareEnabled(true); if (!baseRef) setBaseRef(`${targetRef}^1`); }}>Compare commits</button></div>{compareEnabled && <><label className="revision-input"><span>BASE</span><input aria-label="Base commit" value={baseRef} onChange={e => setBaseRef(e.target.value)} list="commit-options"/></label><button className="swap-button" aria-label="Swap commits" onClick={() => { setBaseRef(targetRef); setTargetRef(baseRef); }}>⇄</button></>}<label className="revision-input"><span>{compareEnabled ? 'TARGET' : 'COMMIT'}</span><input aria-label="Target commit" value={targetRef} onChange={e => setTargetRef(e.target.value)} list="commit-options" onKeyDown={e => { if (e.key === 'Enter') void build(); }}/></label><button className="primary-button small" onClick={() => void build()} disabled={!!pendingJob}>Open graph <span>↗</span></button><button className="text-button history-button" onClick={() => setHistoryOpen(v => !v)}>◷ History</button><span className="revision-note">Immutable source · no checkout required</span><datalist id="commit-options">{refs.map(r => <option key={r.name} value={r.name}>{r.commit.slice(0, 8)}</option>)}{commits.map(c => <option key={c.id} value={c.id}>{c.subject}</option>)}</datalist></div>
    {error && <div role="alert" className="error-banner"><span>{error}</span><button onClick={() => setError('')} aria-label="Dismiss error">×</button></div>}
    {pendingJob && <div className="build-banner" role="status"><span className="spinner"/><strong>Building semantic graph</strong><span>{job?.phase || 'queued'}{job?.counters[job.phase] ? ` · ${job.counters[job.phase].toLocaleString()} items` : ''}</span><span className="build-last">{snapshot ? 'Previous snapshot remains available' : 'Reading committed source with sem'}</span><button className="text-button" onClick={() => api<Job>(`/jobs/${pendingJob}/cancel`, {}).then(setJob).catch(showError)}>Cancel</button></div>}
    <main className="workspace" style={{ gridTemplateColumns: `${ratio}% 5px minmax(0, 1fr)` }}>
      <section className="graph-pane" aria-label="Semantic graph">
        <div className="graph-toolbar"><div className="breadcrumbs"><span className="graph-label">GRAPH</span>{view?.breadcrumbs.map((b, i) => <span key={b.id}>{i > 0 && <em>/</em>}<button className={b.id === scope ? 'current' : ''} onClick={() => { setScope(b.id); setExpanded([]); }}>{b.id === 'root' ? workspace?.name || 'Repository' : b.name}</button></span>)}</div><span className="graph-count">{view?.nodes.length || 0} nodes <i/> {view?.edges.length || 0} links</span></div>
        <div className="graph-tools"><div className="search-box"><span>⌕</span><input ref={searchInput} aria-label="Find module" placeholder="Find a module…" value={searchText} onFocus={() => setSearchOpen(true)} onChange={e => { setSearchText(e.target.value); setSearchOpen(true); }}/><kbd>/</kbd></div><select aria-label="Entity type" value={kind} onChange={e => setKind(e.target.value)}><option value="">All entities</option>{Object.keys(snapshot?.kinds || {}).filter(k => !['directory', 'repository', 'file'].includes(k)).sort().map(k => <option key={k}>{k}</option>)}</select><select aria-label="Relationship type" value={edgeType} onChange={e => setEdgeType(e.target.value)}><option value="">All relations</option><option value="calls">Calls</option><option value="typeref">Type references</option><option value="imports">Imports</option></select><button className={`icon-button ${showMap ? 'active' : ''}`} title="Toggle minimap" aria-label="Toggle minimap" onClick={() => setShowMap(v => !v)}>▧</button></div>
        {comparison && <div className="change-legend"><button className={!changeFilter ? 'active' : ''} onClick={() => setChangeFilter('')}>All changes</button>{Object.entries(palette).filter(([f]) => f !== 'dependencies').map(([f, p]) => <button key={f} className={changeFilter === f ? 'active' : ''} style={{ '--status-color': p.color } as React.CSSProperties} onClick={() => setChangeFilter(old => old === f ? '' : f)}><span>{p.symbol}</span> {p.label} <b>{comparison.summary[f] || 0}</b></button>)}<button className="change-list-button" onClick={() => void changeList().catch(showError)}>List ↗</button>{comparison.diagnostics.length > 0 && <button onClick={() => setInspector({ title: 'Matching notes', detail: comparison.diagnostics.map(d => d.message).join('\n') })}>Matching notes · {comparison.diagnostics.length}</button>}</div>}
        <div className="graph-canvas" onKeyDownCapture={event => {
          const target = event.target as HTMLElement;
          if (!target.classList.contains('react-flow__node') || !['Enter', ' '].includes(event.key)) return;
          const node = view?.nodes.find(n => n.id === target.dataset.id);
          if (!node) return;
          event.preventDefault(); event.stopPropagation();
          if (event.altKey && node.childCount > 0) enter(node); else void selectNode(node);
        }}>
          <ReactFlow nodes={nodes} edges={edges} nodeTypes={nodeTypes} onNodeClick={(_, n) => void selectNode(n.data)} onNodeDoubleClick={(_, n) => n.data.childCount > 0 && enter(n.data)} onEdgeClick={(_, e) => { if (view) api<{ items: EdgeChange[]; total: number }>(`/graph-views/${view.id}/edges/${e.id}?limit=100`).then(r => setInspector({ title: 'Semantic relationships', relations: r.items, total: r.total, detail: `${r.total} references · exact call sites unavailable` })).catch(showError); }} onPaneClick={() => setSearchOpen(false)} onMoveEnd={saveCamera} nodesConnectable={false} nodesDraggable={false} minZoom={.12} maxZoom={2} proOptions={{ hideAttribution: true }} colorMode={theme === 'light' ? 'light' : 'dark'}><Background variant={BackgroundVariant.Dots} gap={24} size={1} color={theme === 'light' ? '#cbd2da' : '#2a3441'}/><Controls showInteractive={false}/>{showMap && <MiniMap nodeColor={n => { const f = primaryFlag((n.data as ModuleData).flags); return f ? palette[f].color : '#7087a5'; }} maskColor={theme === 'dark' ? '#080f18bb' : '#ffffffbb'} pannable zoomable/>}</ReactFlow>
          {!view && <div className="canvas-empty"><div className="empty-symbol">⋈</div><h1>Code, unfolded.</h1><p>{pendingJob ? 'Extracting semantic modules from your commit.' : 'Open a commit to explore its structure.'}</p></div>}
          {view && !view.nodes.length && <div className="canvas-empty"><h2>No modules in this view.</h2><p>Clear a filter or return to a parent container.</p><button onClick={() => { setKind(''); setChangeFilter(''); setScope('root'); }}>Return to overview</button></div>}
          {layoutBusy && view && <div className="layout-status"><span className="spinner"/> Arranging modules</div>}
          {view?.truncated && <div className="limit-notice">{view.omittedNodes} nodes outside the render budget. Enter a container or narrow the view.</div>}
          {routePicking && <div className="route-notice">Select a destination node · {traversalSide} snapshot <button onClick={() => setRoutePicking(false)}>Cancel</button></div>}
          {selected && <div className="selection-tools"><span className="selected-label">{selected.name}</span>{comparison && <select aria-label="Traversal snapshot" value={traversalSide} onChange={e => setTraversalSide(e.target.value as 'base' | 'target')}><option value="target">Target</option><option value="base">Base</option></select>}<button onClick={() => void traverse('dependencies')}>Dependencies →</button><button onClick={() => void traverse('dependents')}>← Dependents</button><button className={routePicking ? 'active' : ''} onClick={() => setRoutePicking(v => !v)}>Route</button></div>}
          {searchOpen && searchText && <div className="search-results" role="listbox" aria-label="Search results">{searchResults.length ? searchResults.map(m => <button role="option" aria-selected="false" key={m.id} onClick={() => void openModule(m)}><span className="result-kind">{m.kind}</span><strong>{m.name}</strong><small>{m.path}{m.source ? `:${m.source.startLine}` : ''}</small></button>) : <div className="search-no-results">No matching modules.</div>}</div>}
          {inspector && <div className="graph-inspector"><div className="inspector-heading"><h3>{inspector.title}</h3><button aria-label="Close inspector" onClick={() => { setInspector(null); setHighlightIds(new Set()); }}>×</button></div><p>{inspector.detail}</p><div className="inspector-items">{inspector.modules?.map(m => <button key={`${m.id}-${m.snapshotSide || inspector.side}`} onClick={() => void openModule(m, m.snapshotSide || inspector.side || (comparison?.changes.find(c => c.before === m.id && !c.after) ? 'base' : 'target'))}><span>{m.kind}</span><strong>{m.name}</strong><small>{m.path}</small></button>)}{inspector.relations?.map((r, i) => <div className="relationship" key={`${r.edge.id}-${i}`}><span style={{ color: r.status === 'added' ? palette.added.color : r.status === 'removed' ? palette.deleted.color : undefined }}>{r.status} · {r.edge.kind}</span><div>{(['from', 'to'] as const).map((end, j) => <span key={end}>{j > 0 && ' → '}<button onClick={() => { const sid = r.side === 'base' && comparison ? comparison.base : snapshot!.id; api<{ module: Module }>(`/snapshots/${sid}/modules/${r.edge[end]}`).then(m => openModule(m.module, r.side)).catch(showError); }}>{end === 'from' ? 'Source entity' : 'Target entity'} ↗</button></span>)}</div></div>)}</div></div>}
          {guide && <div className="guide"><h3>Explore the structure</h3><p>Click a node to read its source. Double-click a container to enter it. Arrows point from a dependent to its dependency.</p><dl><dt>Enter</dt><dd>Select focused node</dd><dt>Alt ↵</dt><dd>Enter selected container</dd><dt>/</dt><dd>Find a module</dd><dt>0</dt><dd>Fit graph</dd><dt>+ / −</dt><dd>Zoom</dd><dt>M</dt><dd>Overview map</dd><dt>Esc</dt><dd>Close overlays</dd></dl><p>Relationships are static references, not runtime execution.</p><button onClick={() => setGuide(false)}>Got it</button></div>}
        </div>
        <footer className="graph-footer"><span><i className="status-dot"/> {snapshot ? 'Snapshot ready' : 'Waiting for snapshot'}</span><span>{snapshot ? `${snapshot.moduleCount.toLocaleString()} modules · ${snapshot.edgeCount.toLocaleString()} dependencies` : 'Powered by sem'}</span><button onClick={() => { if (snapshot) api<{ items: { code: string; message: string }[]; total: number }>(`/snapshots/${snapshot.id}/diagnostics?limit=30`).then(r => setInspector({ title: 'Analysis coverage', detail: `${snapshot.analyzedFiles}/${snapshot.fileCount} files analyzed. ${r.total} diagnostics. ${r.items.map(d => d.message).join('\n')}` })).catch(showError); }}>{snapshot ? `${snapshot.analyzedFiles}/${snapshot.fileCount} files analyzed` : empty}</button></footer>
      </section>
      <div className="splitter" role="separator" aria-label="Resize source pane" aria-orientation="vertical" tabIndex={0} onPointerDown={resize} onKeyDown={e => { if (e.key === 'ArrowLeft') setRatio(v => Math.max(35, v - 2)); if (e.key === 'ArrowRight') setRatio(v => Math.min(78, v + 2)); }}/>
      <CodePane selected={selected} source={source} diff={diff} loading={sourceLoading} theme={theme} onPosition={onCodePosition} onDiffLayout={expanded => setRatio(expanded ? 35 : 66.67)} onExpand={() => selected && enter(selected)}/>
    </main>
    {historyOpen && <div className="history-drawer"><div className="inspector-heading"><h3>Commit history</h3><button aria-label="Close history" onClick={() => setHistoryOpen(false)}>×</button></div><p>Select a local commit. The working tree stays untouched.</p>{commits.map(c => <button className="commit-row" key={c.id} onClick={() => { setTargetRef(c.id); setHistoryOpen(false); }}><code>{c.id.slice(0, 8)}</code><strong>{c.subject}</strong><small>{c.author} · {new Date(c.time * 1000).toLocaleDateString()}</small></button>)}{historyOffset !== null && <button className="text-button" onClick={() => api<{ items: Commit[]; nextOffset: number | null }>(`/commits?${params({ offset: historyOffset, limit: 30 })}`).then(r => { setCommits(cs => [...cs, ...r.items]); setHistoryOffset(r.nextOffset); }).catch(showError)}>Load more commits</button>}</div>}
    <footer className="app-footer"><span>Xfold <b>0.1</b></span><span>{comparison ? 'SEMANTIC COMPARISON' : 'SEMANTIC EXPLORER'}</span><span>{receipt ? `${receipt.stats.mode} · ${(receipt.stats.durationMs / 1000).toFixed(2)}s · ${receipt.stats.filesReused} files reused` : 'Local · private · read only'}</span></footer>
  </div>;
}
