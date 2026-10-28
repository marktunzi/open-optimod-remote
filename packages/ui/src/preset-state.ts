import type { Preset, Snapshot } from './model';

export function modifiedFieldNames(snapshot: Pick<Snapshot, 'modified_fields'>): Set<string> {
  return new Set(snapshot.modified_fields || []);
}

export function optimisticModifiedFields(current: string[] | undefined, fieldName: string): string[] {
  return Array.from(new Set([...(current || []), fieldName])).sort();
}

export function optimisticLessMoreAvailability(current: boolean | undefined, fieldName: string): boolean {
  return fieldName === 'LESS MORE' ? current !== false : false;
}

/** Longest name the model's own PC Remote accepts for a new preset. */
export function maxPresetNameLength(adapterId: string | null | undefined): number {
  return adapterId?.startsWith('pc-remote-5500-') ? 18 : 20;
}

/** Why a new preset name cannot be used, or null when it can. */
export function presetNameProblem(name: string, presets: Preset[], max: number): string | null {
  if (!name) return 'Enter a name.';
  if (name.length > max) return `Use at most ${max} characters.`;
  if (name.trim() !== name) return 'Remove leading and trailing spaces.';
  if (/[\[\]]/.test(name) || /[^\x20-\x7e]/.test(name)) return 'Use plain letters, digits and punctuation without brackets.';
  if (/[\\/:*?"<>|]/.test(name)) return 'A name cannot contain \\ / : * ? " < > or |.';
  if (name.toLowerCase().startsWith('modif ')) return 'A name cannot start with "modif ".';
  const existing = presets.find(p => p.name === name);
  if (existing) return existing.kind === 'Factory' ? 'A factory preset has this name.' : 'A preset with this name already exists.';
  return null;
}

export type PresetManagement = { save: boolean; rename: boolean; delete: boolean; replace: string | null };

/**
 * Which on-device preset actions apply. Save stores the on-air processing;
 * rename needs the selected user preset on air without changes; delete needs
 * a user preset that is not on air.
 */
export function presetManagement(snapshot: Snapshot, selected: string, busy: boolean): PresetManagement {
  // The Save/Delete right of the access level; older services: write_enabled.
  const ready = snapshot.connected && (snapshot.access ? snapshot.access.presets : snapshot.write_enabled) && !busy;
  const store = ready && snapshot.capabilities?.preset_store === true;
  const remove = ready && snapshot.capabilities?.preset_delete === true;
  const onAir = snapshot.processing?.name || '';
  const user = snapshot.presets.some(p => p.name === selected && p.kind === 'User');
  const selectedOnAir = onAir === selected || onAir === `modif ${selected}`;
  return {
    save: store && onAir !== '',
    rename: store && remove && user && onAir === selected,
    delete: remove && user && !selectedOnAir,
    replace: store && replacesOnAir(snapshot.adapter_id) ? onAirUserPreset(snapshot) : null,
  };
}

/** File extension Orban's PC Remote uses for a user preset of this model. */
export function presetFileExtension(adapterId: string | null | undefined): string {
  if (adapterId?.startsWith('pc-remote-5500-')) return 'orb55user';
  if (adapterId?.startsWith('pc-remote-8700hd-')) return 'orb86user';
  if (adapterId?.startsWith('pc-remote-5700i-')) return 'orb57user';
  return 'orb';
}

export type Backup = {
  format: 'open-optimod-backup'; version: number; created: number; model: string;
  adapter_id: string | null; firmware: string;
  processing: { name: string; document: string }; system: { document: string };
  presets: Preset[];
};

/** Parses a backup file and checks that it belongs to the connected model and firmware. */
export function parseBackup(text: string, adapterId: string | null | undefined): Backup {
  let data: Partial<Backup>;
  try { data = JSON.parse(text); } catch { throw new Error('This is not an Open Optimod Remote backup file.'); }
  if (data.format !== 'open-optimod-backup' || data.version !== 1 || typeof data.system?.document !== 'string' || typeof data.processing?.document !== 'string') {
    throw new Error('This is not an Open Optimod Remote backup file.');
  }
  if (!adapterId || data.adapter_id !== adapterId) {
    throw new Error(`This backup was made on ${data.model || 'another processor'} (${data.adapter_id || 'unknown firmware'}). Restore it on the same model and firmware.`);
  }
  return data as Backup;
}

/** Models whose own PC Remote saves the on-air user preset under its own name (5700i Save). */
export function replacesOnAir(adapterId: string | null | undefined): boolean {
  return !!adapterId?.startsWith('pc-remote-5700i-');
}

/** The user preset on air, with or without changes. */
export function onAirUserPreset(snapshot: Snapshot): string | null {
  const onAir = snapshot.processing?.name || '';
  const base = onAir.startsWith('modif ') ? onAir.slice(6) : onAir;
  return snapshot.presets.some(p => p.name === base && p.kind === 'User') ? base : null;
}

/** A file of the original PC Remote's backup folder. */
export type BackupFile = { name: string; document: string; modified?: number };

/** Models whose original PC Remote backs up the unit's own files (setup and user presets). */
export function backsUpFiles(adapterId: string | null | undefined): boolean {
  return !!adapterId?.startsWith('pc-remote-5700i-');
}

/** Sorts files chosen for a restore into the setup file and user preset files; anything else is refused. */
export function sortBackupFiles(files: BackupFile[], adapterId: string | null | undefined): { setup: BackupFile | null; presets: BackupFile[] } {
  const extension = `.${presetFileExtension(adapterId)}`;
  let setup: BackupFile | null = null;
  const presets: BackupFile[] = [];
  for (const file of files) {
    const lower = file.name.toLowerCase();
    if (lower === 'setup.orb57setup' && adapterId?.startsWith('pc-remote-5700i-')) {
      if (setup) throw new Error('Choose one setup file.');
      setup = file;
    } else if (lower.endsWith(extension)) presets.push(file);
    else throw new Error(`${file.name} is not a backup file of this processor.`);
  }
  return { setup, presets };
}

const CRC_TABLE = Array.from({ length: 256 }, (_, n) => {
  let c = n;
  for (let k = 0; k < 8; k++) c = c & 1 ? 0xedb88320 ^ (c >>> 1) : c >>> 1;
  return c >>> 0;
});
function crc32(data: Uint8Array): number {
  let c = 0xffffffff;
  for (const byte of data) c = CRC_TABLE[(c ^ byte) & 0xff] ^ (c >>> 8);
  return (c ^ 0xffffffff) >>> 0;
}

/** An uncompressed ZIP of `files` in a folder named `folder`, as Finder opens it. */
export function zipFiles(folder: string, files: { name: string; data: Uint8Array }[]): Uint8Array {
  const encoder = new TextEncoder();
  const local: Uint8Array[] = [];
  const central: Uint8Array[] = [];
  let offset = 0;
  for (const file of files) {
    const name = encoder.encode(`${folder}/${file.name}`);
    const crc = crc32(file.data);
    const header = new DataView(new ArrayBuffer(30));
    header.setUint32(0, 0x04034b50, true); header.setUint16(4, 20, true);
    header.setUint32(14, crc, true); header.setUint32(18, file.data.length, true); header.setUint32(22, file.data.length, true);
    header.setUint16(26, name.length, true);
    local.push(new Uint8Array(header.buffer), name, file.data);
    const entry = new DataView(new ArrayBuffer(46));
    entry.setUint32(0, 0x02014b50, true); entry.setUint16(4, 20, true); entry.setUint16(6, 20, true);
    entry.setUint32(16, crc, true); entry.setUint32(20, file.data.length, true); entry.setUint32(24, file.data.length, true);
    entry.setUint16(28, name.length, true); entry.setUint32(42, offset, true);
    central.push(new Uint8Array(entry.buffer), name);
    offset += 30 + name.length + file.data.length;
  }
  const size = central.reduce((sum, part) => sum + part.length, 0);
  const end = new DataView(new ArrayBuffer(22));
  end.setUint32(0, 0x06054b50, true); end.setUint16(8, files.length, true); end.setUint16(10, files.length, true);
  end.setUint32(12, size, true); end.setUint32(16, offset, true);
  const parts = [...local, ...central, new Uint8Array(end.buffer)];
  const out = new Uint8Array(parts.reduce((sum, part) => sum + part.length, 0));
  let at = 0;
  for (const part of parts) { out.set(part, at); at += part.length; }
  return out;
}
