import { useRef, useState } from 'react';
import { api } from './api';
import type { Action, Snapshot } from './model';
import { maxPresetNameLength, parseBackup, presetFileExtension, presetManagement, presetNameProblem, type Backup } from './preset-state';

type PendingChange =
  | { kind: 'recall'; name: string; expected_name: string }
  | { kind: 'save'; name: string; expected_name: string }
  | { kind: 'rename'; name: string; new_name: string; expected_name: string }
  | { kind: 'delete'; name: string; expected_name: string }
  | { kind: 'apply-file'; file: string; document: string; expected_name: string }
  | { kind: 'restore'; backup: Backup; file: string; changes: string[]; skipped: { name: string; reason: string }[] };

function download(name: string, text: string, type: string) {
  const url = URL.createObjectURL(new Blob([text], { type }));
  const link = document.createElement('a');
  link.href = url;
  link.download = name;
  link.click();
  URL.revokeObjectURL(url);
}

function safeFilename(name: string) {
  return name.replace(/[^A-Za-z0-9 ._-]/g, '_').trim() || 'preset';
}

export function Presets({ snapshot, busy, action }: { snapshot: Snapshot; busy: boolean; action: Action }) {
  const [query, setQuery] = useState('');
  const [category, setCategory] = useState('All');
  const [selected, setSelected] = useState('');
  const [newName, setNewName] = useState('');
  const [pending, setPending] = useState<PendingChange | null>(null);
  const [notice, setNotice] = useState('');
  const presetInput = useRef<HTMLInputElement>(null);
  const backupInput = useRef<HTMLInputElement>(null);
  const available = snapshot.presets.filter(p => p.kind !== 'Unsaved');
  const visible = available.filter(p => (category === 'All' || p.kind === category) && p.name.toLowerCase().includes(query.toLowerCase()));
  const current = snapshot.processing?.name || '';
  const ready = snapshot.connected && snapshot.write_enabled && !busy;
  const canRecall = ready && available.some(p => p.name === selected);
  const manage = presetManagement(snapshot, selected, busy);
  const onDevice = snapshot.capabilities?.preset_store || snapshot.capabilities?.preset_delete;
  const maxLength = maxPresetNameLength(snapshot.adapter_id);
  const nameProblem = presetNameProblem(newName, snapshot.presets, maxLength);
  const guards = { expected_host: snapshot.host, expected_session: snapshot.session_id, confirmed: true };
  const stale = pending !== null && 'expected_name' in pending && current !== pending.expected_name;

  const confirm = async () => {
    if (!pending || stale) return;
    let done = false;
    switch (pending.kind) {
      case 'recall': done = await action('presets/recall', { name: pending.name, expected_name: pending.expected_name, confirmed: true }); break;
      case 'save': done = await action('presets/manage', { action: 'save', name: pending.name, expected_name: pending.expected_name, ...guards }); break;
      case 'rename': done = await action('presets/manage', { action: 'rename', name: pending.name, new_name: pending.new_name, expected_name: pending.expected_name, ...guards }); break;
      case 'delete': done = await action('presets/manage', { action: 'delete', name: pending.name, expected_name: pending.expected_name, ...guards }); break;
      case 'apply-file': done = await action('presets/apply-file', { document: pending.document, expected_name: pending.expected_name, ...guards }); break;
      case 'restore': done = await action('backup/restore-setup', { document: pending.backup.system.document, ...guards }); break;
    }
    if (done) {
      setNotice(pending.kind === 'restore' ? `Restored ${pending.changes.length} ${pending.changes.length === 1 ? 'setting' : 'settings'} from ${pending.file}.` : '');
      if (pending.kind === 'delete' || pending.kind === 'rename') setSelected(pending.kind === 'rename' ? pending.new_name : '');
      setNewName('');
      setPending(null);
    }
  };

  const savePresetFile = async () => {
    try {
      const file = await api('presets/current-file');
      download(`${safeFilename(file.name)}.${presetFileExtension(snapshot.adapter_id)}`, file.document, 'text/plain');
    } catch (e) { setNotice(e instanceof Error ? e.message : String(e)); }
  };
  const saveBackup = async () => {
    try {
      const backup: Backup = await api('backup');
      const date = new Date(backup.created * 1000).toISOString().slice(0, 10);
      download(`${safeFilename(backup.model)} backup ${date}.json`, JSON.stringify(backup, null, 2), 'application/json');
      setNotice('Backup saved. It holds the system settings, the on-air preset and the names of all presets.');
    } catch (e) { setNotice(e instanceof Error ? e.message : String(e)); }
  };
  const choosePresetFile = async (file: File | undefined) => {
    if (!file) return;
    if (file.size > 65535) { setNotice('This file is too large to be a preset.'); return; }
    setPending({ kind: 'apply-file', file: file.name, document: await file.text(), expected_name: current });
  };
  const chooseBackup = async (file: File | undefined) => {
    if (!file) return;
    try {
      const backup = parseBackup(await file.text(), snapshot.adapter_id);
      const plan = await api('backup/setup-plan', { document: backup.system.document });
      setPending({ kind: 'restore', backup, file: file.name, changes: plan.changes.map((c: { name: string }) => c.name), skipped: plan.skipped });
    } catch (e) { setNotice(e instanceof Error ? e.message : String(e)); }
  };

  return <section className="settings preset-browser" aria-label="Preset library">
    <div className="pageheading"><div><h2>Presets</h2><p>Select a preset, then recall it to put it on air.</p></div>
      <button disabled={!snapshot.connected || busy} onClick={() => void action('refresh', {})}>Refresh presets</button></div>
    <div className="preset-tools"><input name="preset-search" autoComplete="off" aria-label="Search presets" placeholder="Search presets…" value={query} onChange={e => setQuery(e.target.value)} />
      <div className="segmented" aria-label="Preset category">{['All', 'Factory', 'User'].map(c => <button key={c} aria-pressed={category === c} onClick={() => setCategory(c)}>{c}</button>)}</div>
      <span className="muted" role="status">{visible.length} {visible.length === 1 ? 'preset' : 'presets'}</span></div>
    <div className="preset-layout"><div className="preset-list" role="listbox" aria-label="Available presets">
      {visible.map(p =>
        <button key={p.name} role="option" aria-selected={selected === p.name} onClick={() => { setSelected(p.name); setPending(null); }}>
          <strong>{p.name}</strong><span>{p.name === current ? 'On air · ' : ''}{p.kind}</span></button>)}
      {!visible.length && <p className="muted">{!available.length ? (snapshot.connected ? 'No recallable presets returned by the processor.' : 'Connect to load the preset library.') : 'No presets match this search and category.'}</p>}
    </div><aside className="preset-detail"><span className="eyebrow">Selected preset</span><h2>{selected || 'Choose a preset'}</h2>
      <p>On air: <strong>{current || '—'}</strong></p>
      {pending ? <div className="recall-confirmation" role="alert">
        {pending.kind === 'recall' && <p>Replace <strong>{pending.expected_name}</strong> with <strong>{pending.name}</strong>? This changes the on-air sound.</p>}
        {pending.kind === 'save' && <p>Save the on-air processing as the user preset <strong>{pending.name}</strong> on the processor?</p>}
        {pending.kind === 'rename' && <p>Rename the user preset <strong>{pending.name}</strong> to <strong>{pending.new_name}</strong>? The app saves it under the new name, then deletes the old one.</p>}
        {pending.kind === 'delete' && <p>Delete the user preset <strong>{pending.name}</strong> from the processor? This cannot be undone.</p>}
        {pending.kind === 'apply-file' && <p>Apply <strong>{pending.file}</strong> to <strong>{pending.expected_name}</strong>? Every processing value in the file is written and verified. This changes the on-air sound.</p>}
        {pending.kind === 'restore' && <><p>Restore <strong>{pending.changes.length}</strong> system {pending.changes.length === 1 ? 'setting' : 'settings'} from <strong>{pending.file}</strong>? Each change is written once and read back.</p>
          {pending.changes.length > 0 && <p className="muted">{pending.changes.join(', ')}</p>}
          {pending.skipped.length > 0 && <details><summary>{pending.skipped.length} settings are left unchanged</summary>
            <ul>{pending.skipped.map(s => <li key={s.name}><strong>{s.name}</strong>: {s.reason}</li>)}</ul></details>}</>}
        {stale && <p>The on-air preset changed. Review your selection again.</p>}
        <button disabled={!ready || stale || (pending.kind === 'restore' && !pending.changes.length)} onClick={() => void confirm()}>Confirm</button>
        <button disabled={busy} onClick={() => setPending(null)}>Cancel</button></div>
        : <button disabled={!canRecall} onClick={() => setPending({ kind: 'recall', name: selected, expected_name: current })}>Recall preset…</button>}
      <p className="muted">FM and HD use the processing settings stored in this preset. Output routing is controlled separately.</p>

      <h3>On the processor</h3>
      {onDevice ? <>
        <label className="preset-name">New name <input name="preset-name" autoComplete="off" maxLength={maxLength} value={newName} onChange={e => setNewName(e.target.value)} /></label>
        {newName && nameProblem && <p className="muted" role="status">{nameProblem}</p>}
        <div className="preset-actions">
          <button disabled={!manage.save || !!nameProblem || !!pending} onClick={() => setPending({ kind: 'save', name: newName, expected_name: current })}>Save on-air as…</button>
          <button disabled={!manage.rename || !!nameProblem || !!pending} title="Only the selected user preset, on air without changes" onClick={() => setPending({ kind: 'rename', name: selected, new_name: newName, expected_name: current })}>Rename selected…</button>
          <button disabled={!manage.delete || !!pending} title="Only a user preset that is not on air" onClick={() => setPending({ kind: 'delete', name: selected, expected_name: current })}>Delete selected…</button>
        </div>
        <p className="muted">Statically derived from the 5500 firmware and not yet verified on hardware. After every change the app reads the preset list back and reports any difference.</p>
      </> : <p className="muted">{snapshot.connected ? 'Saving, renaming and deleting presets on this processor is not available: its firmware documents no command for it. Save the on-air preset as a file instead.' : 'Connect to manage presets.'}</p>}

      <h3>Files and backup</h3>
      <div className="preset-actions">
        <button disabled={!snapshot.connected || busy} onClick={() => void savePresetFile()}>Save on-air preset as file</button>
        <button disabled={!ready || !!pending} onClick={() => presetInput.current?.click()}>Apply preset file…</button>
        <button disabled={!snapshot.connected || busy} onClick={() => void saveBackup()}>Save backup</button>
        <button disabled={!ready || !!pending} onClick={() => backupInput.current?.click()}>Restore system settings…</button>
      </div>
      <input ref={presetInput} type="file" hidden accept={`.${presetFileExtension(snapshot.adapter_id)},.orb,.txt`} onChange={e => { void choosePresetFile(e.target.files?.[0]); e.target.value = ''; }} />
      <input ref={backupInput} type="file" hidden accept=".json,application/json" onChange={e => { void chooseBackup(e.target.files?.[0]); e.target.value = ''; }} />
      {notice && <p className="muted" role="status">{notice}</p>}
      <p className="muted">A backup holds the system settings, the on-air preset and the names of all presets. The contents of other user presets can only be read by recalling them, so they are not in the backup. A restore never changes network settings or the clock.</p>
    </aside></div>
  </section>;
}
