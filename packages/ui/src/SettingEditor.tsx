import { useState } from 'react';
import { display, type Action, type Definition, type Field } from './model';

export function SettingEditor({ name, scope, field, definition, disabled, expectedSession, action, routing = false }: {
  name: string; scope: 'Processing' | 'System'; field: Field; definition?: Definition;
  disabled: boolean; expectedSession?: string | null; action: Action; routing?: boolean;
}) {
  const [baseline, setBaseline] = useState<Field | null>(null);
  const [baselineSession, setBaselineSession] = useState('');
  const [draft, setDraft] = useState('');
  const text = field.value.type === 'Text';
  const delay = definition?.sample_delay;
  const range = delay ? {min:delay.offset/delay.rate,max:(delay.max_index+delay.offset)/delay.rate} : definition?.integer_range;
  const delayIndex = delay ? Math.round(Number(draft)*delay.rate-delay.offset) : 0;
  const valid = text ? draft.length <= 255 && /^[\x20-\x7e]*$/.test(draft) && !/[;<>]/.test(draft)
    : delay ? draft.trim() !== '' && Number.isFinite(Number(draft)) && Number(draft) >= range!.min && Number(draft) <= range!.max && delayIndex >= 0 && delayIndex <= delay.max_index
    : /^\d+$/.test(draft) && (range
      ? Number(draft) >= range.min && Number(draft) <= range.max
      : Number(draft) < (definition?.values.length || 0));
  const changed = baseline && draft !== String(text || delay ? baseline.value.value : baseline.index);
  const stale = baseline && (JSON.stringify(field) !== JSON.stringify(baseline) || baselineSession !== expectedSession);
  const apply = async () => {
    if (!baseline || !valid || !changed || disabled || stale) return;
    const ok = await action('change', {
      scope, name, index: text ? baseline.index : delay ? delayIndex : Number(draft), expected: baseline,
      expected_session: baselineSession,
      ...(text ? { requested: { type: 'Text', value: draft } } : {}),
    });
    if (ok) setBaseline(null);
  };
  if (!text && !definition) return <span className="muted">Value range not yet verified</span>;
  if (!baseline) return <button disabled={disabled} aria-label={`Edit ${name}`} onClick={() => {
    setBaseline(field); setBaselineSession(expectedSession || ''); setDraft(String(text || delay ? field.value.value : field.index));
  }}>Edit</button>;
  return <div className="setting-editor">
    {text || range ? <input name={`setting-${scope}-${name}`} autoComplete="off" spellCheck={text ? false : undefined} aria-label={`New value for ${name}`} value={draft} disabled={disabled}
      type={range ? 'number' : 'text'} min={range?.min} max={range?.max} step={delay ? 1/delay.rate : range ? 1 : undefined}
      maxLength={255} onChange={e => setDraft(e.target.value)} />
      : <select name={`setting-${scope}-${name}`} aria-label={`New value for ${name}`} value={draft} disabled={disabled} onChange={e => setDraft(e.target.value)}>
        {definition!.values.map((v, i) => <option key={i} value={i}>{display(v, definition?.unit)}</option>)}
      </select>}
    <div className="editor-actions"><button disabled={disabled || !valid || !changed || !!stale} onClick={() => void apply()}>{routing ? 'Apply routing' : 'Apply'}</button>
      <button disabled={disabled} onClick={() => setBaseline(null)}>Cancel</button></div>
    {delay && valid && <small>{((delayIndex + delay.offset) / delay.rate).toFixed(9)} s · nearest supported sample</small>}
    {routing && <small>Changes the signal sent to this physical output.</small>}
    {scope === 'System' && /NETWORK|PORT|IP |GATEWAY|SUBNET/.test(name) && <small>Changing this setting may disconnect the app. Update the saved connection if needed.</small>}
    {stale && <small role="alert">This setting changed elsewhere. Cancel and edit its current value.</small>}
    {!valid && <small role="alert">Enter a supported value{range ? ` (${range.min}–${range.max})` : ''}.</small>}
  </div>;
}
