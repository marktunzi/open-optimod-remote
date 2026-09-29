import { useEffect, useMemo, useState } from "react";
import { api } from "./api";
import {
  filterConnections,
  maskedCode,
  nextSelectionAfterDelete,
  shouldDisconnectBeforeDelete,
} from "./connection-behavior";

export type Device = {
  id: string;
  name: string;
  host: string;
  port: number;
  terminal_port: number;
  has_code: boolean;
  model: ProcessorModel;
};

type ProcessorModel = 'auto' | 'optimod-5700i' | 'optimod-5500i' | 'optimod-5500' | 'optimod-5700-fm' | 'optimod-5700-hd' | 'optimod-8500' | 'optimod-6300' | 'optimod-8600' | 'optimod-8700i' | 'optimod-8700hd' | 'optimod-9300' | 'optimod-9400';

const PROCESSOR_MODELS: { value: ProcessorModel; label: string }[] = [
  { value: 'auto', label: 'Auto Detect' },
  { value: 'optimod-5700i', label: 'OPTIMOD 5700i' },
  { value: 'optimod-5500i', label: 'OPTIMOD 5500i' },
  { value: 'optimod-5500', label: 'OPTIMOD 5500' },
  { value: 'optimod-5700-fm', label: 'OPTIMOD 5700 FM' },
  { value: 'optimod-5700-hd', label: 'OPTIMOD 5700 HD' },
  { value: 'optimod-8500', label: 'OPTIMOD 8500' },
  { value: 'optimod-6300', label: 'OPTIMOD 6300' },
  { value: 'optimod-8600', label: 'OPTIMOD 8600' },
  { value: 'optimod-8700i', label: 'OPTIMOD 8700i' },
  { value: 'optimod-8700hd', label: 'OPTIMOD-FM 8700HD' },
  { value: 'optimod-9300', label: 'OPTIMOD 9300' },
  { value: 'optimod-9400', label: 'OPTIMOD 9400' },
];

type Edit = {
  id?: string;
  name: string;
  host: string;
  port: number;
  terminal_port: number;
  model: ProcessorModel;
};

const blank = (): Edit => ({ name: "", host: "", port: 6201, terminal_port: 23, model: 'auto' });

function ComputerIcon() {
  return <svg viewBox="0 0 24 24" aria-hidden="true"><rect x="3" y="4" width="18" height="12" rx="2"/><path d="M8 20h8M10 16v4m4-4v4"/></svg>;
}

function NetworkIcon() {
  return <svg viewBox="0 0 24 24" aria-hidden="true"><circle cx="12" cy="12" r="9"/><path d="M3 12h18M12 3c3 3.2 3 14.8 0 18M12 3c-3 3.2-3 14.8 0 18"/></svg>;
}

function InfoIcon() {
  return <svg viewBox="0 0 24 24" aria-hidden="true"><circle cx="12" cy="12" r="9"/><path d="M12 10v7M12 7h.01"/></svg>;
}

function GridIcon() {
  return <svg viewBox="0 0 24 24" aria-hidden="true"><rect x="3" y="3" width="7" height="7" rx="1"/><rect x="14" y="3" width="7" height="7" rx="1"/><rect x="3" y="14" width="7" height="7" rx="1"/><rect x="14" y="14" width="7" height="7" rx="1"/></svg>;
}

function ListIcon() {
  return <svg viewBox="0 0 24 24" aria-hidden="true"><path d="M9 6h12M9 12h12M9 18h12"/><circle cx="4.5" cy="6" r="1"/><circle cx="4.5" cy="12" r="1"/><circle cx="4.5" cy="18" r="1"/></svg>;
}

export function Devices({ connected, activeId, activeHost, busy, action, onConnected }: {
  connected: boolean;
  activeId?: string | null;
  activeHost: string;
  busy: boolean;
  action: (name: string, body: unknown, success?: string) => Promise<boolean>;
  onConnected?: () => void;
}) {
  const [devices, setDevices] = useState<Device[]>([]);
  const [selectedId, setSelectedId] = useState(() => localStorage.getItem("optimod-device") || "");
  const [editor, setEditor] = useState<Edit | null>(null);
  const [code, setCode] = useState("");
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState("");
  const [notice, setNotice] = useState("");
  const [confirmDelete, setConfirmDelete] = useState(false);
  const [query, setQuery] = useState("");
  const [view, setView] = useState<"list" | "grid">("list");
  const [source, setSource] = useState<"all" | "network">("all");
  const selected = devices.find(device => device.id === selectedId);
  const active = devices.find(device => device.id === activeId);
  const visible = useMemo(() => filterConnections(devices, query), [devices, query]);
  const locked = busy || saving;

  const rememberSelection = (id: string) => {
    setSelectedId(id);
    if (id) localStorage.setItem("optimod-device", id);
    else localStorage.removeItem("optimod-device");
  };

  const load = async (preferredId?: string) => {
    const result = await api("devices");
    const nextDevices: Device[] = result.devices;
    setDevices(nextDevices);
    const preferred = preferredId ?? selectedId;
    const nextId = nextDevices.some(device => device.id === preferred)
      ? preferred
      : nextDevices[0]?.id ?? "";
    rememberSelection(nextId);
    return nextDevices;
  };

  useEffect(() => {
    void load().catch(reason => setError(reason instanceof Error ? reason.message : String(reason)));
  }, []);

  const choose = (device: Device) => {
    rememberSelection(device.id);
    setEditor(null);
    setCode("");
    setError("");
    setNotice("");
    setConfirmDelete(false);
  };

  const editSelected = (device = selected) => {
    if (!device) return;
    rememberSelection(device.id);
    setEditor({
      id: device.id,
      name: device.name,
      host: device.host,
      port: device.port,
      terminal_port: device.terminal_port,
      model: device.model || 'auto',
    });
    setCode("");
    setError("");
    setConfirmDelete(false);
  };

  const save = async () => {
    if (!editor || locked) return;
    setSaving(true);
    setError("");
    setNotice("");
    try {
      const result = await api("devices", editor);
      const device: Device = result.device;
      if (code) await api("devices/code", { id: device.id, code });
      await load(device.id);
      setEditor(null);
      setCode("");
      setNotice("Connection saved");
    } catch (reason) {
      setError(reason instanceof Error ? reason.message : String(reason));
      await load(editor.id).catch(() => {});
    } finally {
      setSaving(false);
    }
  };

  const connect = async (device = selected) => {
    if (!device || locked) return;
    setSaving(true);
    setError("");
    setNotice("");
    try {
      if (connected && activeId !== device.id && !(await action("disconnect", {}))) return;
      if (code) {
        await api("devices/code", { id: device.id, code });
        await load(device.id);
      }
      const ok = await action("devices/connect", { id: device.id, code: null }, `Connected to ${device.name}`);
      if (ok) {
        setCode("");
        onConnected?.();
      }
    } catch (reason) {
      setError(reason instanceof Error ? reason.message : String(reason));
      await load(device.id).catch(() => {});
    } finally {
      setSaving(false);
    }
  };

  const remove = async () => {
    if (!selected || locked) return;
    setSaving(true);
    setError("");
    setNotice("");
    try {
      if (shouldDisconnectBeforeDelete(connected, activeId, selected.id)) {
        const disconnected = await action("disconnect", {}, "Disconnected");
        if (!disconnected) return;
      }
      const nextId = nextSelectionAfterDelete(devices, selected.id);
      await api("devices/delete", { id: selected.id });
      await load(nextId);
      setEditor(null);
      setCode("");
      setConfirmDelete(false);
      setNotice("Connection deleted");
    } catch (reason) {
      setError(reason instanceof Error ? reason.message : String(reason));
    } finally {
      setSaving(false);
    }
  };

  const showEditor = editor !== null;
  const existing = editor?.id ? devices.find(device => device.id === editor.id) : undefined;

  return <section className="connections" aria-label="Connection manager">
    <aside className="connection-sources" aria-label="Connection sources">
      <div className="traffic-lights" aria-hidden="true"><span/><span/><span/></div>
      <button aria-pressed={source === "all"} onClick={() => setSource("all")}>
        <ComputerIcon/><span>All Connections</span>
      </button>
      <button aria-pressed={source === "network"} onClick={() => setSource("network")}>
        <NetworkIcon/><span>Network</span>
      </button>
      <div className="connection-source-actions">
        <button aria-label="Add connection" disabled={locked} onClick={() => {
          setEditor(blank()); setCode(""); setError(""); setNotice(""); setConfirmDelete(false);
        }}>+</button>
        <button aria-label="Remove selected connection" disabled={locked || !selected} onClick={() => setConfirmDelete(true)}>−</button>
      </div>
    </aside>

    <div className="connection-content">
      <header className="connection-toolbar">
        <h2>{source === "all" ? "All Connections" : "Network"}</h2>
        <div className="connection-view-switch" aria-label="Connection view">
          <button aria-label="Grid view" aria-pressed={view === "grid"} onClick={() => setView("grid")}><GridIcon/></button>
          <button aria-label="List view" aria-pressed={view === "list"} onClick={() => setView("list")}><ListIcon/></button>
        </div>
        <button className="connection-add" aria-label="Add connection" disabled={locked} onClick={() => {
          setEditor(blank()); setCode(""); setError(""); setNotice(""); setConfirmDelete(false);
        }}>+</button>
        <label className="connection-search">
          <span aria-hidden="true">⌕</span>
          <input type="search" value={query} onChange={event => setQuery(event.target.value)} placeholder="Search" aria-label="Search connections"/>
        </label>
      </header>

      {source === "network" ? <div className="connection-empty">
        <NetworkIcon/>
        <h3>Saved local processors</h3>
        <p>Automatic network discovery is not exposed by the 5700i. Add a processor by its IP address.</p>
        <button onClick={() => setEditor(blank())}>Add Optimod</button>
      </div> : <>
        <div className={`connection-list ${view}`} role="listbox" aria-label="Saved Optimods">
          {visible.map(device => {
            const isActive = connected && activeId === device.id;
            return <div key={device.id} className="connection-row" role="option" aria-selected={selectedId === device.id}
              tabIndex={0} onClick={() => choose(device)} onDoubleClick={() => void connect(device)}
              onKeyDown={event => { if (event.key === "Enter") { choose(device); void connect(device); } }}>
              <div className="connection-device-icon"><ComputerIcon/><span className={isActive ? "on" : ""}/></div>
              <div className="connection-name"><strong>{device.name}</strong><span>{PROCESSOR_MODELS.find(model => model.value === device.model)?.label || 'Auto Detect'} · {device.host}</span></div>
              <div className="connection-state">{isActive ? "Connected" : device.has_code ? "Ready" : "Code required"}</div>
              <button className="connection-info" aria-label={`Edit ${device.name}`} disabled={locked}
                onClick={event => { event.stopPropagation(); editSelected(device); }}><InfoIcon/></button>
            </div>;
          })}
          {!visible.length && <div className="connection-empty compact">
            <ComputerIcon/>
            <h3>{devices.length ? "No matching connections" : "Add your first Optimod"}</h3>
            <p>{devices.length ? "Try another name or IP address." : "Save its name, address, ports and access code once."}</p>
          </div>}
        </div>

        {(showEditor || selected) && <section className="connection-inspector" aria-label={showEditor ? "Connection editor" : "Connection details"}>
          {showEditor && editor ? <form onSubmit={event => { event.preventDefault(); void save(); }}>
            <div className="connection-inspector-heading">
              <div><h3>{editor.id ? "Edit connection" : "New Optimod"}</h3><p>Saved only on this Mac.</p></div>
              <button type="button" aria-label="Close editor" onClick={() => { setEditor(null); setCode(""); }}>×</button>
            </div>
            <div className="connection-form-grid">
              <label>Name<input name="connection-name" required maxLength={80} autoFocus autoComplete="off" value={editor.name} disabled={locked} onChange={event => setEditor({ ...editor, name: event.target.value })}/></label>
              <label>Processor<select name="connection-model" value={editor.model} disabled={locked} onChange={event => setEditor({ ...editor, model: event.target.value as ProcessorModel })}>{PROCESSOR_MODELS.map(model => <option key={model.value} value={model.value}>{model.label}</option>)}</select></label>
              <label>IP address<input name="connection-host" required value={editor.host} disabled={locked} placeholder="192.168.1.100" autoComplete="off" spellCheck={false} onChange={event => setEditor({ ...editor, host: event.target.value })}/></label>
              <label>PC Remote port<input name="connection-port" required type="number" min={1} max={65535} value={editor.port} disabled={locked} onChange={event => setEditor({ ...editor, port: Number(event.target.value) })}/></label>
              <label>Status port<input name="status-port" required type="number" min={1} max={65535} value={editor.terminal_port} disabled={locked} onChange={event => setEditor({ ...editor, terminal_port: Number(event.target.value) })}/></label>
              <label className="connection-code-field">Access code<input name="connection-code" type="password" required={!existing?.has_code} value={code} disabled={locked} placeholder={maskedCode(Boolean(existing?.has_code)) || "Enter access code"} autoComplete="off" onChange={event => setCode(event.target.value)}/><small>{existing?.has_code ? "A code is saved. Leave blank to keep it." : "The code is stored privately on this Mac."}</small></label>
            </div>
            <div className="connection-form-actions">
              <button type="button" disabled={locked} onClick={() => { setEditor(null); setCode(""); }}>Cancel</button>
              <button className="primary" disabled={locked}>{saving ? "Saving…" : "Save Connection"}</button>
            </div>
          </form> : selected ? <>
            <div className="connection-summary">
              <div className="connection-device-icon large"><ComputerIcon/><span className={connected && activeId === selected.id ? "on" : ""}/></div>
              <div><h3>{selected.name}</h3><p>{PROCESSOR_MODELS.find(model => model.value === selected.model)?.label || 'Auto Detect'} · {selected.host}:{selected.port} · Status port {selected.terminal_port}</p><small>Access code {maskedCode(selected.has_code) || "not saved"}</small></div>
            </div>
            <div className="connection-detail-actions">
              {connected && activeId === selected.id
                ? <button disabled={locked} onClick={() => void action("disconnect", {}, "Disconnected")}>Disconnect</button>
                : <><label className="quick-code"><span>Access code</span><input type="password" required={!selected.has_code} value={code} disabled={locked} placeholder={maskedCode(selected.has_code) || "Enter code"} onChange={event => setCode(event.target.value)}/></label><button className="primary" disabled={locked || (!selected.has_code && !code)} onClick={() => void connect()}>{saving ? "Connecting…" : "Connect"}</button></>}
              <button disabled={locked} onClick={() => editSelected()}>Edit</button>
              <button className="danger" disabled={locked} onClick={() => setConfirmDelete(true)}>Remove</button>
            </div>
          </> : null}

          {confirmDelete && selected && <div className="connection-delete-confirmation" role="alertdialog" aria-modal="true" aria-label="Remove connection">
            <div><strong>Remove {selected.name}?</strong><p>The saved address and access code will be removed from this Mac.</p></div>
            <button disabled={locked} onClick={() => setConfirmDelete(false)}>Cancel</button>
            <button className="danger" disabled={locked} onClick={() => void remove()}>{saving ? "Removing…" : "Remove"}</button>
          </div>}
        </section>}
      </>}

      {(error || notice) && <div className={`connection-message ${error ? "error" : "notice"}`} role={error ? "alert" : "status"}>{error || notice}</div>}
      {connected && <div className="connection-active-summary"><span className="connection-live-dot"/>Connected to <strong>{active?.name || activeHost}</strong></div>}
    </div>
  </section>;
}
