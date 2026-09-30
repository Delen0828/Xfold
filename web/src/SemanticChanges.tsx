import { useEffect, useState } from 'react';
import { api } from './client';
import { palette } from './palette';
import type { Comparison, Module } from './types';

type ChangeItem = Comparison['changes'][number] & { module: Module };
type Page = { items: ChangeItem[]; total: number };
const pageSize = 50;

export function SemanticChanges({ comparison, selected, onSelect }: {
  comparison: Comparison | null; selected?: string;
  onSelect: (module: Module, side: string) => void;
}) {
  const [page, setPage] = useState<Page | null>(null);
  const [offset, setOffset] = useState(0);
  const [error, setError] = useState('');
  const [loading, setLoading] = useState(false);
  useEffect(() => {
    setPage(null); setError('');
    if (!comparison) return;
    const controller = new AbortController(); setLoading(true);
    api<Page>(`/comparisons/${comparison.id}/changes?offset=${offset}&limit=${pageSize}`, undefined, controller.signal)
      .then(setPage).catch(e => { if (!controller.signal.aborted) setError(String(e)); })
      .finally(() => { if (!controller.signal.aborted) setLoading(false); });
    return () => controller.abort();
  }, [comparison?.id, offset]);
  return <section className="semantic-changes" aria-label="Semantic diff results">
    <header className="pane-title"><span className="eyebrow">SEMANTIC DIFF · SEM</span><span className="readonly">{page ? `${page.total} changes` : 'Commit changes'}</span></header>
    <p className="change-description">Changed entities with parent and child context in the graph.</p>
    <div className="semantic-change-list" aria-busy={loading}>
      {error ? <p role="alert">{error}</p> : loading ? <p role="status">Loading semantic changes…</p> : !page?.items.length ? <p>No semantic changes in this comparison.</p> : page.items.map(change =>
        <button className="semantic-change" key={change.id} aria-pressed={selected === change.id}
          onClick={() => onSelect(change.module, change.after ? 'target' : 'base')}>
          <span className="change-entity"><strong>{change.module.name}</strong><small>{change.module.kind}</small></span>
          <span className="change-file">{change.module.path}{change.module.source ? `:${change.module.source.startLine}` : ''}</span>
          <span className="source-badges">{change.flags.filter(f => f !== 'location').map(f => <span key={f} style={{ color: palette[f]?.color }}>{palette[f]?.symbol} {palette[f]?.label || f}</span>)}</span>
        </button>)}
    </div>
    {page && page.total > pageSize && <nav className="change-pagination" aria-label="Semantic change pages">
      <button disabled={offset === 0 || loading} onClick={() => setOffset(n => Math.max(0, n - pageSize))}>Previous</button>
      <span>{offset + 1}–{Math.min(offset + pageSize, page.total)} of {page.total}</span>
      <button disabled={offset + pageSize >= page.total || loading} onClick={() => setOffset(n => n + pageSize)}>Next</button>
    </nav>}
  </section>;
}
