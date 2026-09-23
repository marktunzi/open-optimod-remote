import { CouplingControl } from './CouplingControl';
import type { Action, Definition, Field, Snapshot } from './model';
import type { ProcessorSkin } from './skin-registry';

function fieldText(snapshot: Snapshot, name: string): string {
  return String(snapshot.system?.fields[name]?.value.value || '');
}

function modulationLabel(snapshot: Snapshot): string {
  const modulation=fieldText(snapshot,'MOD TYPE');
  if(modulation==='L+R') return 'Stereo';
  if(modulation==='L') return 'Mono L';
  if(modulation==='R') return 'Mono R';
  return modulation || 'Mode unavailable';
}

export function InstrumentHeader({snapshot,skin,coupling,couplingDefinition,busy,meterLive,action,onOpenConnections,onOpenPresets,onOpenSetup,activeView}:{
  snapshot: Snapshot;
  skin: ProcessorSkin;
  coupling?: Field;
  couplingDefinition?: Definition;
  busy: boolean;
  meterLive: boolean;
  action: Action;
  onOpenConnections: () => void;
  onOpenPresets: () => void;
  onOpenSetup: () => void;
  activeView: string;
}) {
  const preset=snapshot.processing?.name || 'Unavailable';
  const input=fieldText(snapshot,'ACTUAL A OR D') || 'Input unavailable';
  return <header className="instrument-header">
    <h1 className="sr-only">Open Optimod Remote</h1>
    <img className="optimod-mark" src={skin.logo} alt={skin.productName}/>
    <div className="status-lcd" role="status" aria-live="polite" title={snapshot.processing?.name || ''}>
      <span>ON AIR: {snapshot.connected ? preset : 'Unavailable'}</span>
      <span>{snapshot.connected ? `${input} - ${modulationLabel(snapshot)}` : 'Processor - Offline'}</span>
    </div>
    {snapshot.connected && !coupling ? <section className="coupling-control model-path-indicator" aria-label={`${skin.productName} signal path`}>
      <span className="status-pill" aria-hidden="true"><i/></span><strong>{skin.pathLabel}</strong>
    </section> : <CouplingControl key={snapshot.session_id || 'disconnected'} field={coupling}
      definition={couplingDefinition} connected={snapshot.connected}
      writeEnabled={snapshot.write_enabled} busy={busy} sessionId={snapshot.session_id}
      presetName={snapshot.processing?.name || ''} action={action}/>}
    <button className={`instrument-connection${snapshot.connected?' is-connected':''}${meterLive?' is-live':''}`} title={meterLive?'Live meter data received':snapshot.connected?'Connected; waiting for meter data':'Open connections'} aria-pressed={activeView==='Connections'} onClick={onOpenConnections}>
      <span className="status-pill" aria-hidden="true"><i/></span>
      <strong>{snapshot.connected ? 'Connected' : 'Offline'}</strong>
    </button>
    <button className="instrument-action instrument-setup" aria-pressed={activeView==='Setup'} onClick={onOpenSetup}>
      <span className="status-pill" aria-hidden="true"/>
      <strong>Setup</strong>
    </button>
    <button className="instrument-action instrument-presets" aria-pressed={activeView==='Presets'} onClick={onOpenPresets}>
      <span className="status-pill" aria-hidden="true"/>
      <strong>Presets</strong>
    </button>
    <img className="orban-mark" src="/assets/orban-purple.png" alt="Orban"/>
  </header>;
}
