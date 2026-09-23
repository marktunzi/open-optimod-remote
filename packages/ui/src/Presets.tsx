import { useState } from 'react';
import type { Action, Snapshot } from './model';
export function Presets({ snapshot, busy, action }: { snapshot: Snapshot; busy: boolean; action: Action }) {
  const [query, setQuery] = useState('');
  const [category, setCategory] = useState('All');
  const [selected, setSelected] = useState('');
  const [confirmation, setConfirmation] = useState<{ name: string; expected_name: string } | null>(null);
  const available = snapshot.presets.filter(p => p.kind !== 'Unsaved');
  const visible = available.filter(p => (category === 'All' || p.kind === category) && p.name.toLowerCase().includes(query.toLowerCase()));
  const current = snapshot.processing?.name || '';
  const canRecall = snapshot.connected && snapshot.write_enabled && !busy && available.some(p => p.name === selected);
  const recall = async () => {
    if (!confirmation || !canRecall || current !== confirmation.expected_name) return;
    if (await action('presets/recall', { ...confirmation, confirmed: true })) setConfirmation(null);
  };
  return <section className="settings preset-browser" aria-label="Preset library">
    <div className="pageheading"><div><h2>Presets</h2><p>Select a preset, then recall it to put it on air.</p></div>
      <button disabled={!snapshot.connected || busy} onClick={() => void action('refresh', {})}>Refresh presets</button></div>
    <div className="preset-tools"><input name="preset-search" autoComplete="off" aria-label="Search presets" placeholder="Search presets…" value={query} onChange={e => setQuery(e.target.value)} />
      <div className="segmented" aria-label="Preset category">{['All', 'Factory', 'User'].map(c => <button key={c} aria-pressed={category === c} onClick={() => setCategory(c)}>{c}</button>)}</div>
      <span className="muted" role="status">{visible.length} {visible.length === 1 ? 'preset' : 'presets'}</span></div>
    <div className="preset-layout"><div className="preset-list" role="listbox" aria-label="Available presets">
      {visible.map(p =>
        <button key={p.name} role="option" aria-selected={selected === p.name} onClick={() => { setSelected(p.name); setConfirmation(null); }}>
          <strong>{p.name}</strong><span>{p.name === current ? 'On air · ' : ''}{p.kind}</span></button>)}
      {!visible.length && <p className="muted">{!available.length ? (snapshot.connected ? 'No recallable presets returned by the processor.' : 'Connect to load the preset library.') : 'No presets match this search and category.'}</p>}
    </div><aside className="preset-detail"><span className="eyebrow">Selected preset</span><h2>{selected || 'Choose a preset'}</h2>
      <p>On air: <strong>{current || '—'}</strong></p>
      {confirmation ? <div className="recall-confirmation" role="alert"><p>Replace <strong>{confirmation.expected_name}</strong> with <strong>{confirmation.name}</strong>? This changes the on-air sound.</p>
        {current !== confirmation.expected_name && <p>The on-air preset changed. Review your selection again.</p>}
        <button disabled={!canRecall || current !== confirmation.expected_name} onClick={() => void recall()}>Confirm recall</button>
        <button disabled={busy} onClick={() => setConfirmation(null)}>Cancel</button></div>
        : <button disabled={!canRecall} onClick={() => setConfirmation({ name: selected, expected_name: current })}>Recall preset…</button>}
      <p className="muted">FM and HD use the processing settings stored in this preset. Output routing is controlled separately.</p>
    </aside></div>
  </section>;
}
