export const palette: Record<string, { color: string; symbol: string; label: string }> = {
  added: { color: '#58cb96', symbol: '⊕', label: 'Added' },
  modified: { color: '#e7ba67', symbol: '∆', label: 'Modified' },
  deleted: { color: '#ef828b', symbol: '⊖', label: 'Deleted' },
  moved: { color: '#80aafa', symbol: '→', label: 'Moved' },
  renamed: { color: '#6ed2df', symbol: '↻', label: 'Renamed' },
  reordered: { color: '#c493eb', symbol: '↕', label: 'Reordered' },
  cosmetic: { color: '#919bab', symbol: '~', label: 'Cosmetic' },
  dependencies: { color: '#b0bad2', symbol: '◇', label: 'Dependencies' },
};
export const primaryFlag = (flags: string[]) => ['deleted', 'added', 'moved', 'renamed', 'modified', 'reordered', 'cosmetic', 'dependencies'].find(f => flags.includes(f));
