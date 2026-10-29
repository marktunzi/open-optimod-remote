import { useEffect, useState } from 'react';
import { FigmaIcon, type FigmaIconName } from './FigmaIcon';
import { CouplingControl } from './CouplingControl';
import type { Action, Definition, Field, Snapshot } from './model';
import type { ProcessorSkin } from './skin-registry';

function ActionButton({ icon, label, onClick, disabled, pressed }: {
  icon: FigmaIconName; label: string; onClick: () => void; disabled?: boolean; pressed?: boolean;
}) {
  return <button className="fx-action" onClick={onClick} disabled={disabled} aria-pressed={pressed}>
    <span className="fx-action-face"><FigmaIcon name={icon}/></span>
    <span className="fx-action-label">{label}</span>
  </button>;
}

function text(snapshot: Snapshot, name: string): string {
  return String(snapshot.system?.fields[name]?.value.value ?? '');
}

/** The Figma clock format: "02:53:39 PM Oct, 10, 2026". */
export function figmaClock(now: Date): string {
  const time = now.toLocaleTimeString('en-US', { hour: '2-digit', minute: '2-digit', second: '2-digit', hour12: true });
  const month = now.toLocaleDateString('en-US', { month: 'short' });
  return `${time} ${month}, ${String(now.getDate()).padStart(2, '0')}, ${now.getFullYear()}`;
}

/** "OPTIMOD-FM 8700HD" → "8700HD". */
export function modelShortName(productName: string): string {
  return productName.replace(/^OPTIMOD(-FM)?\s+/i, '') || productName;
}

/**
 * The Meters window header of the Figma design (frame 150:2690): a status
 * strip and an action bar. Every model shows the same design; only the values
 * come from the connected processor.
 */
export function FigmaMetersHeader({ snapshot, skin, deviceName, meterLive, coupling, couplingDefinition, busy, action,
  onOpenSetup, onOpenPresets, onOpenProcessing, onOpenConnections }: {
  snapshot: Snapshot; skin: ProcessorSkin; deviceName: string | null; meterLive: boolean;
  coupling?: Field; couplingDefinition?: Definition; busy: boolean; action: Action;
  onOpenSetup: () => void; onOpenPresets: () => void; onOpenProcessing: () => void; onOpenConnections: () => void;
}) {
  const [now, setNow] = useState(() => new Date());
  useEffect(() => {
    const timer = setInterval(() => setNow(new Date()), 1000);
    return () => clearInterval(timer);
  }, []);
  const connected = snapshot.connected;
  const state = connected ? 'Connected' : snapshot.reconnect ? 'Reconnecting' : snapshot.error ? 'Connection lost' : 'Offline';
  const automation = text(snapshot, 'AUTOMATION ENABLED');
  // Offline, only what identifies the processor stays: no stale preset or status.
  const cells = (connected ? [
    modelShortName(skin.productName),
    deviceName,
    snapshot.processing?.name,
    text(snapshot, 'ACTUAL A OR D') || null,
    snapshot.host,
    automation ? `Automation: ${automation}` : null,
  ] : [modelShortName(skin.productName), deviceName, snapshot.host]).filter((cell): cell is string => Boolean(cell));
  return <header className="fx-header">
    <h1 className="sr-only">Meters</h1>
    <div className="fx-status" role="status" aria-live="polite">
      <button className={`fx-cell fx-connection is-${state.split(' ')[0].toLowerCase()}${meterLive ? ' is-live' : ''}`}
        onClick={onOpenConnections} title={snapshot.error ?? (meterLive ? 'Live meter data received' : 'Open connections')}>
        <i aria-hidden="true"/>{state}
      </button>
      {cells.map((cell, index) => <span className="fx-cell" key={index}>{cell}</span>)}
      <span className="fx-cell fx-clock" title="Time on this Mac">{figmaClock(now)}</span>
    </div>
    <div className="fx-bar">
      <img className="fx-logo" src="/assets/orban.png" alt="Orban"/>
      <nav className="fx-actions" aria-label="Processor">
        <ActionButton icon="setup" label="Setup" onClick={onOpenSetup} disabled={!connected}/>
        <ActionButton icon="presets" label="Presets" onClick={onOpenPresets}/>
        {coupling && couplingDefinition && <CouplingControl variant="figma" key={snapshot.session_id || 'disconnected'}
          field={coupling} definition={couplingDefinition} connected={connected} writeEnabled={snapshot.write_enabled}
          busy={busy} sessionId={snapshot.session_id} presetName={snapshot.processing?.name || ''} action={action}/>}
        <ActionButton icon="recall" label="Recall" onClick={onOpenPresets} disabled={!connected || snapshot.capabilities?.preset_recall === false}/>
        <ActionButton icon="settings" label="Settings" onClick={onOpenProcessing}/>
      </nav>
    </div>
  </header>;
}
