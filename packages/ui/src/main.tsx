import React, { useState, useEffect, useRef, useCallback } from "react";
import { createRoot } from "react-dom/client";
import { api } from "./api";
import { MeterStrip } from "./MeterStrip";
import { REFERENCE_METERS, meterModelFromProfile, type MeterModel, type ProfileMeters } from "./meters";
import { Devices } from "./Devices";
import layouts from "./layouts.json";
import "./style.css";
import "./main-interface.css";
import { type Field, type Snapshot, type Definition, display } from './model';
import { SettingEditor } from './SettingEditor';
import { Presets } from './Presets';
import { Outputs } from './Outputs';
import { Setup } from './Setup';
import { InstrumentHeader } from './InstrumentHeader';
import { DEFAULT_METER_VIEW, initialWorkspace, processingControlEditable, processingPages, processingPathSelectorVisible, systemSettingsTarget, type WorkspaceView } from './navigation-model';
import { binaryOnIndex, groupProcessingPage, type LayoutPage } from './processing-layout';
import { modifiedFieldNames, optimisticLessMoreAvailability, optimisticModifiedFields } from './preset-state';
import { nextControlIndex } from './control-keyboard';
import { resolveProcessorSkin } from './skin-registry';
import { ModelMeterPanel } from './ModelMeterPanel';
declare global {
  interface Window {
    webkit?: { messageHandlers?: {
      openSystemSettings?: { postMessage: (body: unknown) => void };
      openConnections?: { postMessage: (body: unknown) => void };
      openPresets?: { postMessage: (body: unknown) => void };
      recordError?: { postMessage: (body: unknown) => void };
    } };
  }
}
function reportError(source: string, error: unknown) {
  const message = error instanceof Error ? error.message : String(error);
  window.webkit?.messageHandlers?.recordError?.postMessage({ source, message });
  console.error(`[${source}] ${message}`);
}
const empty: Snapshot = {
  connected: false,
  host: "",
  firmware: "",
  processing: null,
  system: null,
  error: null,
  revision: 0,
  session_id: null,
  presets: [],
  modified_fields: [],
  less_more_available: false,
  write_enabled: false,
  model: 'auto',
  skin_id: null,
  adapter_id: null,
  capabilities: null,
};
function Control({
  name,
  label,
  field,
  definition,
  disabled,
  pending = false,
  modified = false,
  change,
}: {
  name: string;
  label: string;
  field?: Field;
  definition?: Definition;
  disabled: boolean;
  pending?: boolean;
  modified?: boolean;
  change: (index: number) => void;
}) {
  const [draft, setDraft] = useState<number | null>(null);
  const dragging = useRef(false);
  const range = useRef<HTMLInputElement>(null);
  const current = draft ?? field?.index ?? 0;
  const available = !disabled && !!definition && !!field;
  const commit = (index: number) => {
    setDraft(null);
    if (available && !pending && index !== field!.index) change(index);
  };
  useEffect(() => {
    if (!dragging.current) setDraft(null);
  }, [field?.index, disabled]);
  useEffect(() => {
    const element = range.current;
    if (!element) return;
    const wheel = (e: WheelEvent) => {
      if (document.activeElement !== element || !available) return;
      e.preventDefault();
      if (!pending)
        commit(
          Math.max(
            0,
            Math.min(
              definition!.values.length - 1,
              current + (e.deltaY < 0 ? 1 : -1),
            ),
          ),
        );
    };
    element.addEventListener("wheel", wheel, { passive: false });
    return () => element.removeEventListener("wheel", wheel);
  }, [available, pending, current, definition, field]);
  return (
    <div
      className={`control ${!available ? "unavailable" : ""}${modified ? " modified" : ""}`}
      title={
        !field
          ? "Not available in this preset"
          : !definition
            ? "Value range is not yet verified"
            : label
      }
    >
      <label>{label}</label>
      <div className="controlrow">
        <input
          ref={range}
          name={`processing-${name}`}
          type="range"
          aria-label={label}
          aria-disabled={!available || pending}
          min={0}
          max={Math.max(0, (definition?.values.length || 1) - 1)}
          value={current}
          disabled={!available}
          onPointerDown={() => {
            dragging.current = !pending;
          }}
          onPointerUp={(e) => {
            dragging.current = false;
            commit(Number(e.currentTarget.value));
          }}
          onPointerCancel={() => {
            dragging.current = false;
            setDraft(null);
          }}
          onChange={(e) => {
            if (pending) return;
            const v = Number(e.target.value);
            if (dragging.current) setDraft(v);
            else commit(v);
          }}

          onKeyDown={(e) => {
            if (
              pending &&
              [
                "ArrowLeft",
                "ArrowRight",
                "ArrowUp",
                "ArrowDown",
                "+",
                "-",
                "Home",
                "End",
              ].includes(e.key)
            ) {
              e.preventDefault();
              return;
            }
            if (e.key === "+" || e.key === "-") {
              e.preventDefault();
              commit(
                Math.max(
                  0,
                  Math.min(
                    definition!.values.length - 1,
                    current + (e.key === "+" ? 1 : -1),
                  ),
                ),
              );
            }
          }}
        />
        <input
          className="control-value"
          aria-label={`${label} value`}
          aria-valuemin={0}
          aria-valuemax={Math.max(0, (definition?.values.length || 1) - 1)}
          aria-valuenow={current}
          role="spinbutton"
          readOnly
          disabled={!available}
          value={display(draft !== null ? definition?.values[draft] : field?.value, definition?.unit)}
          onKeyDown={(event) => {
            const next = nextControlIndex(current, Math.max(0, definition!.values.length - 1), event.key);
            if (next === null) return;
            event.preventDefault();
            if (!pending) commit(next);
          }}
        />
      </div>
      <span className="sr-only">{name}</span>
    </div>
  );
}

function BinaryToggle({ name, label, options, field, disabled, pending, modified, change }: {
  name: string;
  label: string;
  options: string[];
  field?: Field;
  disabled: boolean;
  pending: boolean;
  modified: boolean;
  change: (index: number) => void;
}) {
  const onIndex = binaryOnIndex(options);
  if (onIndex === null) return null;
  const checked = field?.index === onIndex;
  const otherIndex = onIndex === 0 ? 1 : 0;
  return <div className={`control toggle-control ${disabled || !field ? 'unavailable' : ''}${modified ? ' modified' : ''}`}>
    <label htmlFor={`processing-${name}`}>{label}</label>
    <div className="toggle-row">
      <button id={`processing-${name}`} type="button" role="switch" aria-checked={checked}
        className={`toggle-switch ${checked ? 'on' : ''}`} disabled={disabled || !field || pending}
        onClick={() => change(checked ? otherIndex : onIndex)}><span/></button>
      <output>{field ? options[field.index] ?? String(field.value.value) : 'Unavailable'}</output>
    </div>
    <span className="sr-only">{name}</span>
  </div>;
}
function App() {
  const [snapshot, setSnapshot] = useState<Snapshot>(empty);
  const [definitions, setDefinitions] = useState<Definition[]>([]);
  const [modelLayouts, setModelLayouts] = useState<LayoutPage[] | null>(null);
  const [meterModel, setMeterModel] = useState<MeterModel>(REFERENCE_METERS);
  const [path, setPath] = useState<"FM" | "HD">("FM");
  const [area, setArea] = useState<WorkspaceView>(() => initialWorkspace(false, Boolean(window.webkit?.messageHandlers?.openConnections)));
  const [tab, setTab] = useState("AGC");
  const [busy, setBusy] = useState(false);
  const [filter, setFilter] = useState("");
  const [meterLive, setMeterLive] = useState(false);
  const mutationInFlight = useRef(false);
  const lastReportedDeviceError = useRef<string | null>(null);
  const load = useCallback(async () => {
    const s = await api("state");
    setSnapshot(s);
  }, []);
  useEffect(() => {
    void load().catch(e => reportError('Interface', e));
    const timer = setInterval(
      () =>
        !mutationInFlight.current && void load().catch(() =>
          setSnapshot((s) => ({
            ...s,
            connected: false,
            error: "The local app is not reachable",
          })),
        ),
      1000,
    );
    return () => clearInterval(timer);
  }, [load]);
  // Each processor profile carries its own parameters and, for models beyond
  // the 5700i reference, its own processing pages and meters.
  useEffect(() => {
    void api("profile")
      .then((p: { fields: Definition[]; layouts?: LayoutPage[]; meters?: ProfileMeters }) => {
        setDefinitions(p.fields);
        setModelLayouts(p.layouts ?? null);
        setMeterModel(p.meters ? meterModelFromProfile(p.meters) : REFERENCE_METERS);
      })
      .catch((e) => reportError('Parameter profile', e));
  }, [snapshot.adapter_id]);
  useEffect(() => {
    if (snapshot.error && snapshot.error !== lastReportedDeviceError.current) {
      lastReportedDeviceError.current = snapshot.error;
      reportError('Optimod', snapshot.error);
    } else if (!snapshot.error) {
      lastReportedDeviceError.current = null;
    }
  }, [snapshot.error]);
  const action = async (name: string, body: unknown) => {
    if (busy) return false;
    mutationInFlight.current = true;
    setBusy(true);
    try {
      const request = name === 'change' || name === 'presets/recall'
        ? { expected_host: snapshot.host, expected_session: snapshot.session_id, expected_preset: snapshot.processing?.name, ...(body as object) }
        : body;
      await api(name, request);
      await load();
      return true;
    } catch (e) {
      reportError(name, e);
      await load().catch(() => {});
      return false;
    } finally {
      mutationInFlight.current = false;
      setBusy(false);
    }
  };
  const coupling = snapshot.processing?.fields["HD COUPLING"];
  const couplingValue = String(coupling?.value.value || '') || undefined;
  const pages = processingPages(modelLayouts ?? (layouts as LayoutPage[]), path, couplingValue);
  const page = pages.find((p) => p.title === tab) || pages[0];
  const processingGroups = groupProcessingPage(page);
  const independentPaths = processingPathSelectorVisible(couplingValue);
  useEffect(() => {
    if (!independentPaths && path !== 'FM') setPath('FM');
  }, [independentPaths, path]);
  const definition = (scope: string, name: string) =>
    definitions.find((d) => d.scope === scope && d.name === name);
  const change = (
    scope: "Processing" | "System",
    name: string,
    field: Field | undefined,
    index: number,
  ) => {
    if (!field) return;
    const selected = definition(scope, name)?.values[index];
    if (selected) {
      setSnapshot(current => {
        const document = scope === 'Processing' ? current.processing : current.system;
        if (!document) return current;
        const updated = { ...document, fields: { ...document.fields, [name]: { index, value: selected } } };
        return scope === 'Processing'
          ? {
              ...current,
              processing: updated,
              modified_fields: optimisticModifiedFields(current.modified_fields, name),
              less_more_available: optimisticLessMoreAvailability(current.less_more_available, name),
            }
          : { ...current, system: updated };
      });
    }
    void action("change", { scope, name, index, expected: field });
  };
  const records = snapshot.processing?.fields;
  const modifiedFields = modifiedFieldNames(snapshot);
  const skin = resolveProcessorSkin(snapshot.skin_id);
  const lessMoreAvailable = snapshot.less_more_available !== false;
  useEffect(() => {
    if (!lessMoreAvailable && tab === 'Less More') setTab('Stereo Enhancer');
  }, [lessMoreAvailable, tab]);
  const openSystemSettings = () => {
    const handler = window.webkit?.messageHandlers?.openSystemSettings;
    // The native Settings window follows the 5700i worksheet; models with their
    // own profile use the generic Setup workspace that lists every system field.
    if (systemSettingsTarget(Boolean(handler) && modelLayouts === null) === 'native') handler!.postMessage({ source: '5700i-interface' });
    else setArea('Setup');
  };
  const openConnections = () => {
    const handler = window.webkit?.messageHandlers?.openConnections;
    if (handler) handler.postMessage({ source: '5700i-interface' });
    else setArea('Connections');
  };
  const openPresets = () => {
    const handler = window.webkit?.messageHandlers?.openPresets;
    if (handler) handler.postMessage({ source: '5700i-interface' });
    else setArea('Presets');
  };
  return (
    <main className={area === 'Connections' ? 'connection-mode' : ''} data-skin={skin.id} data-material={skin.material}>
      <a className="skip-link" href="#main-workspace">Skip to Controls</a>
      {area === 'Connections' ? <div className="connection-workspace" id="main-workspace" tabIndex={-1}>
        <Devices connected={snapshot.connected} activeId={snapshot.device_id} activeHost={snapshot.host}
          busy={busy} action={action} onConnected={() => setArea('Processing')}/>
      </div> : <>
      <InstrumentHeader snapshot={snapshot} skin={skin} coupling={coupling}
        couplingDefinition={definition('Processing','HD COUPLING')} busy={busy}
        meterLive={meterLive} action={action} activeView={area}
        onOpenConnections={openConnections}
        onOpenPresets={openPresets}
        onOpenSetup={openSystemSettings}/>
      {snapshot.connected && snapshot.capabilities?.live_meters === false
        ? <ModelMeterPanel skin={skin}/>
        : <MeterStrip connected={snapshot.connected} identity={snapshot.session_id || snapshot.device_id || snapshot.host}
            view={DEFAULT_METER_VIEW} onLiveChange={setMeterLive} model={meterModel}/>}
      <div className="app-body">
      <div className="workspace" id="main-workspace" tabIndex={-1}>
      {snapshot.connected && !snapshot.write_enabled && <div className="development">Read-only access: this connection does not allow changes.</div>}
      {snapshot.connected && snapshot.write_enabled && snapshot.evidence === 'static' && <div className="development">Statically derived profile, not yet verified on this processor. Every change is confirmed by reading the processor back.</div>}
      {area === "Presets" ? <Presets key={snapshot.session_id || snapshot.device_id || snapshot.host} snapshot={snapshot} busy={busy} action={action} /> : area === "Outputs" ? <Outputs key={snapshot.session_id || snapshot.device_id || snapshot.host} snapshot={snapshot} definitions={definitions} busy={busy} action={action} /> : area === "Processing" ? (
        <>
          <nav
            className="tabs"
            aria-label="Processing page"
            onKeyDown={(e) => {
              if (e.ctrlKey && e.key === "Tab") {
                e.preventDefault();
                const i = pages.findIndex((p) => p.title === page.title);
                setTab(
                  pages[
                    (i + (e.shiftKey ? -1 : 1) + pages.length) % pages.length
                  ].title,
                );
              }
            }}
          >
            {pages.map((p) => (
              <button
                key={p.title}
                aria-pressed={page.title === p.title}
                disabled={p.title === 'Less More' && !lessMoreAvailable}
                title={p.title === 'Less More' && !lessMoreAvailable ? 'Recall a factory preset to use Less More' : undefined}
                onClick={() => setTab(p.title)}
              >
                {p.title === 'Equalizer' ? 'EQ' : p.title === 'Speech Mode' ? 'Speech mode' : p.title}
              </button>
            ))}
            {independentPaths && <div className="processing-path-selector segmented" aria-label="Processing path">
              {(["FM", "HD"] as const).map(processingPath => <button key={processingPath}
                aria-pressed={path === processingPath} onClick={() => setPath(processingPath)}>{processingPath}</button>)}
            </div>}
          </nav>
          <section className="processing" aria-label={page.title}>
            <div className={`processing-groups ${processingGroups.length === 1 ? 'single-group-page' : ''} ${page.title === 'Band Mix' ? 'band-mix-page' : ''}`}>
              {processingGroups.map(group => <section className={`processing-group ${group.className || ''}`} key={group.title}>
                <h3>{group.title}</h3>
                <div className={`control-well ${group.controls.length > 10 ? 'dense' : ''}`}>
                  {group.controls.map(control => {
                    const field = snapshot.processing?.fields[control.name];
                    const controlDefinition = definition("Processing", control.name);
                    const disabled = !snapshot.connected || !snapshot.write_enabled
                      || !processingControlEditable(control.name, couplingValue);
                    const toggleIndex = control.kind === 'radio' ? binaryOnIndex(control.options) : null;
                    return <div className="processing-control" key={control.name}>
                      {control.kind === 'radio' && toggleIndex !== null ? <BinaryToggle
                        name={control.name} label={control.label} options={control.options}
                        field={field} pending={busy} disabled={disabled || !controlDefinition}
                        modified={modifiedFields.has(control.name)}
                        change={index => change("Processing", control.name, field, index)}/>
                      : control.kind === 'radio' ? <fieldset className="radio-control" disabled={disabled || !field || !controlDefinition}>
                        <legend>{control.label}</legend>
                        {control.options.map((option, index) => <label key={option}>
                          <input type="radio" name={control.name} checked={field?.index === index}
                            aria-disabled={busy} onChange={() => { if (!busy) change("Processing", control.name, field, index); }}/>
                          {option}
                        </label>)}
                      </fieldset> : <Control name={control.name} label={control.label}
                        field={field} definition={controlDefinition} pending={busy} disabled={disabled}
                        modified={modifiedFields.has(control.name)}
                        change={index => change("Processing", control.name, field, index)}/>} 
                    </div>;
                  })}
                </div>
              </section>)}
            </div>
          </section>
        </>
      ) : area === "Setup" ? (
        <Setup records={snapshot.system?.fields} definitions={definitions} connected={snapshot.connected}
          writeEnabled={snapshot.write_enabled} busy={busy} identity={snapshot.device_id || snapshot.host}
          sessionId={snapshot.session_id} action={action} openOutputs={()=>setArea('Outputs')} />
      ) : (
        <section className="settings">
          <div className="pageheading">
            <h2>All Processing Parameters</h2>
            <input
              name="parameter-search"
              autoComplete="off"
              aria-label="Search parameters"
              placeholder="Search settings…"
              value={filter}
              onChange={(e) => setFilter(e.target.value)}
            />
          </div>
          <table>
            <thead>
              <tr>
                <th>Setting</th>
                <th>Current value</th>
                <th>Control</th>
              </tr>
            </thead>
            <tbody>
              {Object.entries(records || {})
                .filter(([n]) => n.toLowerCase().includes(filter.toLowerCase()))
                .map(([n, f]) => (
                  <tr key={n}>
                    <td>{n}</td>
                    <td>{display(f.value, definition("Processing", n)?.unit)}</td>
                    <td>
                      <SettingEditor
                        key={`${snapshot.device_id || snapshot.host}:${snapshot.session_id}:${snapshot.processing?.name}:${n}`}
                        name={n} scope="Processing" field={f}
                        definition={definition("Processing", n)}
                        disabled={!snapshot.connected || !snapshot.write_enabled || busy || !processingControlEditable(n, couplingValue)}
                        expectedSession={snapshot.session_id}
                        action={action}
                      />
                    </td>
                  </tr>
                ))}
            </tbody>
          </table>
        </section>
      )}
      </div>
      </div>
      </>}
    </main>
  );
}
createRoot(document.getElementById("root")!).render(
  <React.StrictMode>
    <App />
  </React.StrictMode>,
);
