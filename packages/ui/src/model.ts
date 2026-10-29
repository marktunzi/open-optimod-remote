export type Value = { type: 'Int' | 'Cent' | 'Choice' | 'Text'; value: number | string };
export type Field = { index: number; value: Value };
export type Doc = { name: string; fields: Record<string, Field> };
export type Preset = { name: string; kind: 'Factory' | 'User' | 'Unsaved' };
export type Capabilities = {
  connect: boolean;
  parameter_reads: boolean;
  parameter_writes: boolean;
  live_meters: boolean;
  preset_catalog: boolean;
  preset_recall: boolean;
  /** Save the on-air processing as a user preset on the processor. */
  preset_store?: boolean;
  /** Delete a user preset on the processor. */
  preset_delete?: boolean;
};
/** What the session may change at its access level (the client's Security levels). */
export type Access = { level: number; recall: boolean; modify: boolean; presets: boolean; setup: boolean;
  automation: boolean; security: boolean; memory: boolean };
/** One right of the session; a service without `access` grants all with `write_enabled`. */
export function allows(snapshot: { connected: boolean; write_enabled: boolean; access?: Access | null }, right: keyof Omit<Access, 'level'>): boolean {
  return snapshot.connected && (snapshot.access ? snapshot.access[right] : snapshot.write_enabled);
}
export type Snapshot = {
  access?: Access | null;
  connected: boolean; host: string; firmware: string;
  processing: Doc | null; system: Doc | null; error: string | null;
  revision: number; device_id?: string | null; session_id?: string | null; presets: Preset[]; write_enabled: boolean;
  preset_base_name?: string | null; modified_fields: string[]; less_more_available?: boolean;
  model?: string; skin_id?: string | null; adapter_id?: string | null; capabilities?: Capabilities | null;
  evidence?: 'hardware' | 'static' | 'none' | null;
  /** Connected, but the current settings could not be read from the status port. */
  sync_error?: string | null;
  /** Set while the app waits to reconnect after an unexpected session loss. */
  reconnect?: import('./reconnect-status').ReconnectStatus | null;
};
export type Definition = {
  scope: 'Processing' | 'System'; name: string; values: Value[]; unit?: string;
  sample_delay?: { max_index: number; offset: number; rate: number };
  integer_range?: { min: number; max: number };
  evidence?: string;
};
export type Action = (name: string, body: unknown) => Promise<boolean>;
export const display = (v?: Value, unit?: string) => {
  if (!v) return '—';
  const value = v.type === 'Cent'
    ? (Number(v.value) / 100).toLocaleString('en-US', { maximumFractionDigits: 2 })
    : String(v.value);
  return unit && (v.type === 'Int' || v.type === 'Cent') ? `${value} ${unit}` : value;
};
