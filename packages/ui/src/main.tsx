import React, { useState, useEffect, useRef, useCallback, useMemo } from "react";
import { createRoot } from "react-dom/client";
import { api } from "./api";
import { MeterStrip } from "./MeterStrip";
import { REFERENCE_METERS, meterModelFromProfile, settingValues, type MeterModel, type ProfileMeters } from "./meters";
import { Devices } from "./Devices";
import layouts from "./layouts.json";
import "./style.css";
import "./main-interface.css";
import { allows, type Field, type Snapshot, type Definition, display } from './model';
import { reconnectMessage } from './reconnect-status';
import { SettingEditor } from './SettingEditor';
import { Presets } from './Presets';
import { Outputs } from './Outputs';
import { Setup } from './Setup';
import { InstrumentHeader } from './InstrumentHeader';
import { FigmaMetersHeader } from './FigmaMetersHeader';
import { DEFAULT_METER_VIEW, initialWorkspace, pageControls, shownProcessingPages, tabLabel, processingControlEditable, processingPathSelectorVisible, systemSettingsTarget, type WorkspaceView } from './navigation-model';
import { binaryOnIndex, groupProcessingPage, readingOrder, type LayoutPage } from './processing-layout';
import { modifiedFieldNames, optimisticLessMoreAvailability, optimisticModifiedFields } from './preset-state';
import { nextControlIndex } from './control-keyboard';
import { resolveProcessorSkin } from './skin-registry';
import { ModelMeterPanel } from './ModelMeterPanel';
import { deviceBinding, parseWindowRoute, scopedSnapshot, type WindowKind } from './window-route';
declare global {
  interface Window {
    webkit?: { messageHandlers?: {
      openSystemSettings?: { postMessage: (body: unknown) => void };
      openConnections?: { postMessage: (body: unknown) => void };
      openPresets?: { postMessage: (body: unknown) => void };
      openMeters?: { postMessage: (body: unknown) => void };
      openProcessing?: { postMessage: (body: unknown) => void };
      recordError?: { postMessage: (body: unknown) => void };
    } };
    /** Called by the macOS app menu (Command-,) so it opens the right settings for the connected model. */
    openOptimodSystemSettings?: () => void;
    /** Called by the macOS app menu (Shift-Command-P) to open the Presets workspace. */
    openOptimodPresets?: () => void;
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
  const valueText = display(draft !== null ? definition?.values[draft] : field?.value, definition?.unit);
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
          className={`control-value${valueText.length > 8 ? ' long-value' : ''}`}
          title={valueText}
          aria-label={`${label} value`}
          aria-valuemin={0}
          aria-valuemax={Math.max(0, (definition?.values.length || 1) - 1)}
          aria-valuenow={current}
          role="spinbutton"
          readOnly
          disabled={!available}
          value={valueText}
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

/**
 * A text box of the original client (the preset RDS page): the firmware's
 * `undefined` shows empty, as in the client, and an emptied box writes
 * `undefined` back, as the client does. Enter or leaving the box writes;
 * Escape restores the current value. The write is confirmed by readback.
 */
export function TextControl({ name, label, field, maxLength, disabled, pending, modified, write }: {
  name: string; label: string; field?: Field; maxLength?: number; disabled: boolean; pending: boolean;
  modified: boolean; write: (text: string) => Promise<boolean>;
}) {
  const current = field?.value.type === 'Text' && field.value.value !== 'undefined' ? String(field.value.value) : '';
  const [draft, setDraft] = useState<string | null>(null);
  useEffect(() => setDraft(null), [current]);
  const value = draft ?? current;
  const valid = value.length <= (maxLength ?? 255) && /^[\x20-\x7e]*$/.test(value) && !/[;<>]/.test(value);
  const available = !disabled && !!field;
  const commit = async () => {
    if (draft === null || draft === current || !valid || !available || pending) return;
    if (!(await write(draft === '' ? 'undefined' : draft))) setDraft(null);
  };
  return <div className={`control text-control${!available ? ' unavailable' : ''}${modified ? ' modified' : ''}`}
    title={!field ? 'Not available in this preset' : label}>
    <label htmlFor={`processing-${name}`}>{label}</label>
    <input id={`processing-${name}`} name={`processing-${name}`} className="control-value text-value" type="text"
      autoComplete="off" spellCheck={false} maxLength={maxLength ?? 255} value={value} disabled={!available}
      aria-invalid={!valid} aria-busy={pending}
      onChange={event => setDraft(event.target.value)}
      onBlur={() => void commit()}
      onKeyDown={event => {
        if (event.key === 'Enter') { event.preventDefault(); void commit(); }
        if (event.key === 'Escape') { event.preventDefault(); setDraft(null); }
      }}/>
    {!valid && <small role="alert">Letters, digits and spaces only; no ; &lt; or &gt;.</small>}
    <span className="sr-only">{name}</span>
  </div>;
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
/** The first view of each native window; the browser starts on Processing. */
function initialArea(kind: WindowKind | 'combined', nativeConnections: boolean): WorkspaceView {
  if (kind === 'presets') return 'Presets';
  if (kind === 'processing' || kind === 'meters') return 'Processing';
  return initialWorkspace(false, nativeConnections);
}
function App() {
  // In the macOS app every window is its own page on one processor.
  const route = useMemo(() => parseWindowRoute(window.location.search), []);
  const windowed = route.kind !== 'combined';
  const [sessionSnapshot, setSnapshot] = useState<Snapshot>(empty);
  const binding = deviceBinding(route, sessionSnapshot);
  const snapshot = scopedSnapshot(route, sessionSnapshot, empty);
  const [definitions, setDefinitions] = useState<Definition[]>([]);
  const [modelLayouts, setModelLayouts] = useState<LayoutPage[] | null>(null);
  const [supplementLayouts, setSupplementLayouts] = useState<LayoutPage[]>([]);
  const [meterModel, setMeterModel] = useState<MeterModel>(REFERENCE_METERS);
  const [path, setPath] = useState<"FM" | "HD">("FM");
  const [area, setArea] = useState<WorkspaceView>(() => initialArea(route.kind, Boolean(window.webkit?.messageHandlers?.openConnections)));
  const [tab, setTab] = useState("AGC");
  const [busy, setBusy] = useState(false);
  const [filter, setFilter] = useState("");
  const [meterLive, setMeterLive] = useState(false);
  const [deviceNames, setDeviceNames] = useState<Record<string, string>>({});
  useEffect(() => {
    if (!windowed || !sessionSnapshot.device_id || deviceNames[sessionSnapshot.device_id]) return;
    void api('devices').then((book: { devices: { id: string; name: string }[] }) =>
      setDeviceNames(Object.fromEntries(book.devices.map(device => [device.id, device.name]))))
      .catch(e => reportError('Connections', e));
  }, [windowed, sessionSnapshot.device_id, deviceNames]);
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
      .then((p: { fields: Definition[]; layouts?: LayoutPage[]; supplement_layouts?: LayoutPage[]; meters?: ProfileMeters }) => {
        setDefinitions(p.fields);
        setModelLayouts(p.layouts ?? null);
        setSupplementLayouts(p.supplement_layouts ?? []);
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
  const liveFields = snapshot.processing?.fields;
  const meterSettings = useMemo(() => settingValues(snapshot.processing, snapshot.system), [snapshot.revision, snapshot.processing?.name]);
  const pages = shownProcessingPages(modelLayouts, layouts as LayoutPage[], supplementLayouts, liveFields, path, couplingValue);
  const selectedPage = pages.find((p) => p.title === tab) || pages[0];
  // Extracted pages take the original dialog's order (the 5700i's built-in
  // pages keep the Figma order); the page's switch then goes first.
  const page = selectedPage && { ...selectedPage, controls: pageControls(
    selectedPage.reference_dialog ? readingOrder(selectedPage.controls) : selectedPage.controls, liveFields,
    control => control.kind === 'radio' && binaryOnIndex(control.options) !== null) };
  const processingGroups = page ? groupProcessingPage(page) : [];
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
    // A native window asks the app, which opens this processor's own System Settings window.
    if (windowed && handler) { handler.postMessage({ device: route.deviceId }); return; }
    // The native Settings window follows the 5700i worksheet; models with their
    // own profile use the generic Setup workspace that lists every system field.
    if (systemSettingsTarget(Boolean(handler) && modelLayouts === null) === 'native') handler!.postMessage({ source: '5700i-interface' });
    else setArea('Setup');
  };
  useEffect(() => {
    if (windowed) return;
    window.openOptimodSystemSettings = openSystemSettings;
    window.openOptimodPresets = () => setArea('Presets');
    return () => { delete window.openOptimodSystemSettings; delete window.openOptimodPresets; };
  });
  const openConnections = () => {
    const handler = window.webkit?.messageHandlers?.openConnections;
    if (handler) handler.postMessage({ source: '5700i-interface' });
    else setArea('Connections');
  };
  // The Presets workspace holds recall, on-device save/rename/delete, preset
  // files and backups for every model, so the macOS app uses it as well.
  const openPresets = () => {
    const handler = window.webkit?.messageHandlers?.openPresets;
    if (windowed && handler) handler.postMessage({ device: route.deviceId });
    else setArea('Presets');
  };
  // Shown in every view: after a lost session the app may wait minutes.
  const reconnectBanner = !snapshot.connected && snapshot.reconnect
    ? <div className="development" role="status">{reconnectMessage(snapshot.reconnect, Date.now())} <button disabled={busy || snapshot.reconnect.in_progress} onClick={() => void api('reconnect/cancel', {}).then(load).catch(e => reportError('Cancel reconnect', e))}>Cancel</button></div>
    : null;
  // A window of one processor while another one is connected (Phase 6 keeps both).
  const otherDeviceNotice = <div className="development window-other-device" role="status">
    This window belongs to another processor, which is not connected. Its data is not shown here.
  </div>;
  return (
    <main className={area === 'Connections' ? 'connection-mode' : ''}
      data-skin={windowed ? 'figma' : skin.id} data-material={windowed ? 'figma' : skin.material}
      data-window={route.kind} data-binding={binding}>
      <a className="skip-link" href="#main-workspace">Skip to Controls</a>
      {area === 'Connections' ? <div className="connection-workspace" id="main-workspace" tabIndex={-1}>
        {reconnectBanner}
        <Devices connected={snapshot.connected} activeId={snapshot.device_id} activeHost={snapshot.host}
          busy={busy} action={action} onConnected={() => setArea('Processing')}/>
      </div> : <>
      {route.kind === 'meters' && <FigmaMetersHeader snapshot={snapshot} skin={skin}
        deviceName={snapshot.device_id ? deviceNames[snapshot.device_id] ?? null : null} meterLive={meterLive}
        coupling={coupling} couplingDefinition={definition('Processing','HD COUPLING')} busy={busy} action={action}
        onOpenSetup={openSystemSettings} onOpenPresets={openPresets} onOpenConnections={openConnections}
        onOpenProcessing={() => window.webkit?.messageHandlers?.openProcessing?.postMessage({ device: route.deviceId })}/>}
      {(route.kind === 'combined' || route.kind === 'meters') && <>
      {route.kind === 'combined' && <InstrumentHeader snapshot={snapshot} skin={skin} coupling={coupling}
        couplingDefinition={definition('Processing','HD COUPLING')} busy={busy}
        meterLive={meterLive} action={action} activeView={area}
        onOpenConnections={openConnections}
        onOpenPresets={openPresets}
        onOpenSetup={openSystemSettings}/>}
      {snapshot.connected && snapshot.capabilities?.live_meters === false
        ? <ModelMeterPanel skin={skin}/>
        : <MeterStrip connected={snapshot.connected} identity={snapshot.session_id || snapshot.device_id || snapshot.host}
            view={DEFAULT_METER_VIEW} onLiveChange={setMeterLive} model={meterModel} values={meterSettings}
            theme={windowed ? 'figma' : 'instrument'}/>}
      </>}
      {route.kind === 'meters' && (binding === 'other-device' ? otherDeviceNotice : reconnectBanner)}
      {route.kind !== 'meters' && <div className="app-body">
      <div className="workspace" id="main-workspace" tabIndex={-1}>
      {binding === 'other-device' && otherDeviceNotice}
      {reconnectBanner}
      {windowed && !snapshot.connected && !snapshot.reconnect && binding !== 'other-device' && <div className="fx-status window-offline" role="status">
        <span className="fx-cell fx-connection"><i aria-hidden="true"/>Not connected</span>
        <button type="button" className="fx-offline-open" onClick={openConnections}>Connections…</button></div>}

      {snapshot.connected && snapshot.sync_error && <div className="development" role="status">{snapshot.sync_error} <button disabled={busy} onClick={() => void action('refresh', {})}>Refresh</button></div>}
      {snapshot.connected && !snapshot.write_enabled && <div className="development">Read-only access: this connection does not allow changes.</div>}
      {area === "Presets" ? <Presets key={snapshot.session_id || snapshot.device_id || snapshot.host} snapshot={snapshot} busy={busy} action={action} /> : area === "Outputs" ? <Outputs key={snapshot.session_id || snapshot.device_id || snapshot.host} snapshot={snapshot} definitions={definitions} busy={busy} action={action} /> : area === "Processing" ? (!page ? (
        <section className="processing processing-unknown" role="status" aria-label="Processing">
          {snapshot.connected ? "The processing appears once the processor's on-air settings have been read." : "Connect to a processor to see its processing."}
        </section>
      ) : (
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
                title={p.title === 'Less More' && !lessMoreAvailable ? 'Recall a factory preset to use Less More' : p.title}
                onClick={() => setTab(p.title)}
              >
                {tabLabel(p.title)}
              </button>
            ))}
            {independentPaths && <div className="processing-path-selector segmented" aria-label="Processing path">
              {(["FM", "HD"] as const).map(processingPath => <button key={processingPath}
                aria-pressed={path === processingPath} onClick={() => setPath(processingPath)}>{processingPath}</button>)}
            </div>}
          </nav>
          <section className="processing" aria-label={page.title}>
            <div className={`processing-groups ${processingGroups.length === 1 && !page.sections ? 'single-group-page' : ''} ${page.title === 'Band Mix' && !page.sections ? 'band-mix-page' : ''} ${page.sections ? 'sectioned-page' : ''}`}>
              {processingGroups.map(group => <section className={`processing-group ${group.className || ''}`} key={group.title}>
                <h3>{group.title}</h3>
                <div className={`control-well ${group.controls.length > 10 ? 'dense' : ''}`}>
                  {group.controls.map((control, controlIndex) => {
                    const field = snapshot.processing?.fields[control.name];
                    const controlDefinition = definition("Processing", control.name);
                    const disabled = !snapshot.connected || !snapshot.write_enabled
                      || !processingControlEditable(control.name, couplingValue);
                    const toggleIndex = control.kind === 'radio' ? binaryOnIndex(control.options) : null;
                    // Only the page's on/off switch, put first, gets the outlined box of the design.
                    const pageSwitch = controlIndex === 0 && toggleIndex !== null;
                    return <div className={`processing-control${pageSwitch ? ' page-switch' : ''}`} key={control.name}>
                      {control.kind === 'text' ? <TextControl name={control.name} label={control.label}
                        field={field} maxLength={control.max_length} pending={busy} disabled={disabled}
                        modified={modifiedFields.has(control.name)}
                        write={text => action('change', { scope: 'Processing', name: control.name, index: field!.index,
                          expected: field, requested: { type: 'Text', value: text } })}/>
                      : control.kind === 'radio' && toggleIndex !== null ? <BinaryToggle
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
      )) : area === "Setup" ? (
        <Setup records={snapshot.system?.fields} definitions={definitions} connected={snapshot.connected}
          writeEnabled={allows(snapshot, 'setup')} busy={busy} identity={snapshot.device_id || snapshot.host}
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
      </div>}
      </>}
    </main>
  );
}
createRoot(document.getElementById("root")!).render(
  <React.StrictMode>
    <App />
  </React.StrictMode>,
);
