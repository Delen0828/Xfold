import { useEffect, useLayoutEffect, useRef, useState } from 'react';
import * as monaco from 'monaco-editor/esm/vs/editor/editor.api.js';
import './languages';
import EditorWorker from 'monaco-editor/esm/vs/editor/editor.worker?worker';
import type { SourceDiff, SourceResponse, ViewNode } from './types';
import { palette } from './palette';
// The local ESM editor never calls a CDN or loads a language service remotely.
self.MonacoEnvironment = { getWorker: () => new EditorWorker() };
// Monaco contributions cancel pending link/hover requests when their editor is
// disposed. Some 0.55 contributions leave that normal cancellation unhandled.
// Preserve reporting for every other error, including genuine worker failures.
addEventListener('unhandledrejection', event => {
  const error = event.reason;
  if (error instanceof Error && error.name === 'Canceled' && error.message === 'Canceled') event.preventDefault();
});

// Own model lifetime explicitly: detach models before disposal. Keeping model
// ownership here also avoids retaining every file visited in a long session.
function CommittedEditor({ before, after, compare, options, theme, mount, mountDiff }: {
  before?: SourceResponse | null; after?: SourceResponse | null; compare?: boolean;
  options: monaco.editor.IStandaloneDiffEditorConstructionOptions;
  theme: string; mount: (e: monaco.editor.IStandaloneCodeEditor) => void;
  mountDiff: (e: monaco.editor.IStandaloneDiffEditor) => void;
}) {
  const host = useRef<HTMLDivElement>(null);
  useLayoutEffect(() => {
    if (!host.current) return;
    const modified = monaco.editor.createModel(after?.text || '', after?.language || before?.language || 'plaintext');
    if (compare) {
      const original = monaco.editor.createModel(before?.text || '', before?.language || after?.language || 'plaintext');
      const editor = monaco.editor.createDiffEditor(host.current, options);
      const viewModel = editor.createViewModel({ original, modified });
      editor.setModel(viewModel); mountDiff(editor);
      // A detached widget may still have a worker diff in flight. Let that
      // bounded computation settle before releasing its models and worker.
      return () => { void viewModel.waitForDiff().finally(() => { editor.setModel(null); editor.dispose(); viewModel.dispose(); original.dispose(); modified.dispose(); }); };
    }
    const editor = monaco.editor.create(host.current, { ...options, model: modified }); mount(editor);
    return () => { editor.setModel(null); editor.dispose(); modified.dispose(); };
  }, [before, after, compare, options.renderSideBySide]);
  useEffect(() => { monaco.editor.setTheme(theme === 'light' ? 'vs' : 'vs-dark'); }, [theme]);
  return <div ref={host} style={{ height: '100%', width: '100%' }}/>;
}

type Props = { selected: ViewNode | null; source: SourceResponse | null; diff: SourceDiff | null; loading: boolean; theme: string; onPosition: (source: SourceResponse, line: number, byte: number) => void; onExpand: () => void; onDiffLayout: (expanded: boolean) => void };
export function CodePane({ selected, source, diff, loading, theme, onPosition, onExpand, onDiffLayout }: Props) {
  const [tab, setTab] = useState<'diff' | 'before' | 'after'>('diff');
  const [sideBySide, setSideBySide] = useState(false);
  const editor = useRef<monaco.editor.IStandaloneCodeEditor | null>(null);
  const diffEditor = useRef<monaco.editor.IStandaloneDiffEditor | null>(null);
  const decorations = useRef<monaco.editor.IEditorDecorationsCollection | null>(null);
  const active = diff ? (tab === 'before' ? diff.before : diff.after) : source;
  const activeRef = useRef(active); activeRef.current = active;
  const positionRef = useRef(onPosition); positionRef.current = onPosition;
  const options: monaco.editor.IStandaloneEditorConstructionOptions = { readOnly: true, domReadOnly: true, minimap: { enabled: false }, fontSize: 12, fontFamily: '"SFMono-Regular", Consolas, monospace', lineHeight: 21, scrollBeyondLastLine: false, padding: { top: 15 }, automaticLayout: true, renderLineHighlight: 'gutter', wordWrap: 'off', glyphMargin: false, folding: true, contextmenu: false };
  const highlight = (e: monaco.editor.IStandaloneCodeEditor, value: SourceResponse | null | undefined) => {
    decorations.current?.clear();
    if (value?.anchor) {
      const a = value.anchor;
      decorations.current = e.createDecorationsCollection([{ range: new monaco.Range(a.startLine, 1, a.endLine, 1), options: { isWholeLine: true, className: 'entity-source-highlight', linesDecorationsClassName: 'entity-source-gutter' } }]);
      e.revealLineInCenter(a.startLine);
    }
  };
  useEffect(() => { if (editor.current) highlight(editor.current, active); }, [active, selected, tab]);
  useEffect(() => {
    if (diffEditor.current && diff) {
      const d = diffEditor.current;
      if (diff.after?.anchor) d.getModifiedEditor().revealLineInCenter(diff.after.anchor.startLine);
      if (diff.before?.anchor) d.getOriginalEditor().revealLineInCenter(diff.before.anchor.startLine);
    }
  }, [diff]);
  useEffect(() => { setTab('diff'); }, [selected?.id]);
  const navigateFrom = (e: monaco.editor.IStandaloneCodeEditor, valueForEditor: () => SourceResponse | null | undefined) => {
    const listener = e.onMouseDown(event => {
      const value = valueForEditor(); const p = event.target.position;
      if (!value || !p || !event.event.ctrlKey && !event.event.metaKey) return;
      const text = value.text.split('\n').slice(0, p.lineNumber - 1).join('\n');
      const prefix = text + (p.lineNumber > 1 ? '\n' : '') + (value.text.split('\n')[p.lineNumber - 1] || '').slice(0, p.column - 1);
      positionRef.current(value, p.lineNumber, new TextEncoder().encode(prefix).length);
    });
    e.onDidDispose(() => listener.dispose());
  };
  const mount = (e: monaco.editor.IStandaloneCodeEditor) => {
    editor.current = e; highlight(e, activeRef.current);
    navigateFrom(e, () => activeRef.current);
    e.onDidDispose(() => { if (editor.current === e) editor.current = null; });
  };
  return <section className="code-pane" aria-label="Source code">
    <header className="pane-title"><span className="eyebrow">SOURCE</span><span className="readonly"><i/> Read only</span><button className="icon-button" onClick={() => { setSideBySide(!sideBySide); onDiffLayout(!sideBySide); }} title="Toggle side-by-side source diff" aria-label="Toggle side-by-side source diff">◫</button></header>
    {!selected ? <div className="empty-source"><div className="source-mark">{'{ }'}</div><h2>Follow the code.</h2><p>Select a module to see its source.<br/>Every line belongs to the selected commit.</p><div className="key-hint"><kbd>/</kbd> find a module</div></div> : <>
      <div className="source-heading"><div className="eyebrow">{selected.kind}</div><h2 title={selected.name}>{selected.name}</h2><div className="source-path" title={selected.path}>{selected.path || 'Repository overview'}</div><div className="source-badges">{selected.flags.filter(f => f !== 'location').map(f => <span key={f} style={{ color: palette[f]?.color }}>{palette[f]?.symbol} {palette[f]?.label || f}</span>)}</div></div>
      {diff?.before && (diff.before.path !== diff.after?.path || diff.beforeName !== diff.afterName) && diff.after && <div className="previous-source">Previously <strong>{diff.beforeName}</strong><br/>{diff.before.path} · L{diff.before.anchor?.startLine || 1}</div>}
      {diff && <div className="source-tabs" role="tablist">{(['diff', 'before', 'after'] as const).map(t => <button key={t} role="tab" aria-selected={tab === t} onClick={() => setTab(t)}>{t === 'diff' ? 'Changes' : t === 'before' ? 'Before' : 'After'}</button>)}</div>}
      <div className="source-meta"><span>{diff ? 'COMMIT COMPARISON' : selected.snapshot.slice(0, 8)}</span><span>{active?.anchor ? `L${active.anchor.startLine}–${active.anchor.endLine}` : selected.source ? `L${selected.source.startLine}–${selected.source.endLine}` : `${selected.childCount} children`}</span></div>
      <div className="editor-area">
        {loading ? <div className="source-loading">Loading committed source…</div> : !selected.source ? <div className="container-summary"><h3>{selected.kind === 'file' ? 'Source unavailable' : `${selected.childCount} child modules`}</h3><p>{selected.kind === 'file' ? 'This file has no supported text source. See coverage diagnostics for its status.' : `${selected.descendantCount} descendants in this container.`}</p>{selected.childCount > 0 && <button className="primary-button" onClick={onExpand}>Explore this container →</button>}</div> : diff && tab === 'diff' ? <CommittedEditor key={`${selected.id}-diff`} before={diff.before} after={diff.after} compare theme={theme} options={{ ...options, renderSideBySide: sideBySide, originalEditable: false, enableSplitViewResizing: true }} mount={mount} mountDiff={e => { highlight(diff.after ? e.getModifiedEditor() : e.getOriginalEditor(), diff.after || diff.before); navigateFrom(e.getOriginalEditor(), () => diff.before); navigateFrom(e.getModifiedEditor(), () => diff.after); diffEditor.current = e; e.onDidDispose(() => { if (diffEditor.current === e) diffEditor.current = null; }); if (diff.after?.anchor) e.getModifiedEditor().revealLineInCenter(diff.after.anchor.startLine); else if (diff.before?.anchor) e.getOriginalEditor().revealLineInCenter(diff.before.anchor.startLine); }}/>
          : active ? <CommittedEditor key={`${selected.id}-${tab}`} after={active} theme={theme} options={options} mount={mount} mountDiff={() => {}}/>
          : <div className="source-loading">This module does not exist in the {tab === 'before' ? 'base' : 'target'} commit.</div>}
      </div>
      <footer className="source-footer"><span>Git snapshot · UTF-8</span><span>⌘ / Ctrl + click to focus entity</span></footer>
    </>}
  </section>;
}
