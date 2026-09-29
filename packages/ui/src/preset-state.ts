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
  if (name.toLowerCase().startsWith('modif ')) return 'A name cannot start with "modif ".';
  const existing = presets.find(p => p.name === name);
  if (existing) return existing.kind === 'Factory' ? 'A factory preset has this name.' : 'A preset with this name already exists.';
  return null;
}

export type PresetManagement = { save: boolean; rename: boolean; delete: boolean };

/**
 * Which on-device preset actions apply. Save stores the on-air processing;
 * rename needs the selected user preset on air without changes; delete needs
 * a user preset that is not on air.
 */
export function presetManagement(snapshot: Snapshot, selected: string, busy: boolean): PresetManagement {
  const ready = snapshot.connected && snapshot.write_enabled && !busy;
  const store = ready && snapshot.capabilities?.preset_store === true;
  const remove = ready && snapshot.capabilities?.preset_delete === true;
  const onAir = snapshot.processing?.name || '';
  const user = snapshot.presets.some(p => p.name === selected && p.kind === 'User');
  const selectedOnAir = onAir === selected || onAir === `modif ${selected}`;
  return {
    save: store && onAir !== '',
    rename: store && remove && user && onAir === selected,
    delete: remove && user && !selectedOnAir,
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
