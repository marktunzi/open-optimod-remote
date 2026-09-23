import { SettingEditor } from './SettingEditor';
import { display, type Action, type Definition, type Snapshot } from './model';
const outputs = [['AO1 SOURCE', 'Analog L/R'], ['DO1 SOURCE', 'Digital 1 · AES3'], ['DO2 SOURCE', 'Digital 2 · AES3'], ['PHONES OUT SOURCE', 'Headphones']];
export function Outputs({ snapshot, definitions, busy, action }: { snapshot: Snapshot; definitions: Definition[]; busy: boolean; action: Action }) {
  return <section className="settings" aria-label="Physical output routing"><h2>Outputs</h2>
    <p>Choose the processing source for each physical output. To use FM and HD together, assign them to different outputs.</p>
    <div className="output-grid">{outputs.map(([name, label]) => {
      const field = snapshot.system?.fields[name];
      return <section className="output-card" key={name}><span className="eyebrow">{label}</span><h2>{display(field?.value)}</h2>
        {field ? <SettingEditor name={name} scope="System" field={field} definition={definitions.find(d => d.scope === 'System' && d.name === name)} disabled={!snapshot.connected || !snapshot.write_enabled || busy} expectedSession={snapshot.session_id} action={action} routing /> : <p className="muted">Connect to read this output.</p>}
      </section>;
    })}</div>
    <p>FM+Delay includes the diversity delay. Monitor selects the low-latency monitor path. The composite outputs carry the FM multiplex signal.</p>
    <p className="muted">The FM / HD view buttons only select what is displayed. Use Apply routing here to change an output.</p>
  </section>;
}
