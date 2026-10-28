import type { MeterView } from './meters';
import type { LayoutPage } from './processing-layout';

export const TOP_WORKSPACES = ['Processing', 'Presets', 'Connections'] as const;
export type WorkspaceView = (typeof TOP_WORKSPACES)[number] | 'Outputs' | 'Setup' | 'All parameters';

export const DEFAULT_METER_VIEW: MeterView = 'Both';

export function defaultWorkspace(connected: boolean): WorkspaceView {
  return connected ? 'Processing' : 'Connections';
}

export function initialWorkspace(connected: boolean, hasNativeConnections: boolean): WorkspaceView {
  return hasNativeConnections ? 'Processing' : defaultWorkspace(connected);
}

export function processingPathSelectorVisible(coupling: string | undefined): boolean {
  return coupling !== undefined && coupling !== 'FM->HD';
}

const UNIQUE_HD_LIMITING_CONTROLS = new Set([
  'IBOC EQ GAIN',
  'IBOC EQ FREQ',
  'IBOC LIM DR',
  'HD DE ESS',
  'HD COUPLING',
]);

export function processingControlEditable(name: string, coupling: string | undefined): boolean {
  return coupling !== 'FM->HD'
    || !name.startsWith('HD ')
    || UNIQUE_HD_LIMITING_CONTROLS.has(name);
}

export function processingPages(
  layouts: LayoutPage[],
  path: 'FM' | 'HD',
  _coupling: string | undefined,
): LayoutPage[] {
  return layouts.filter(page =>
    page.path === 'Shared'
    || page.path === path
    || (page.path === 'HD' && page.title === 'HD Limiting'),
  );
}

export function systemSettingsTarget(hasNativeBridge: boolean): 'native' | 'workspace' {
  return hasNativeBridge ? 'native' : 'workspace';
}

/**
 * Adds statically derived pages to a model's built-in pages. A supplementary
 * page is skipped when a built-in page on the same path has its title, or
 * already covers every one of its controls.
 */
export function mergeSupplementPages(builtin: LayoutPage[], supplement: LayoutPage[]): LayoutPage[] {
  const added = supplement.filter(page => {
    const samePath = builtin.filter(existing => existing.path === page.path);
    if (samePath.some(existing => existing.title === page.title)) return false;
    const covered = new Set(samePath.flatMap(existing => existing.controls.map(control => control.name)));
    return page.controls.some(control => !covered.has(control.name));
  });
  return [...builtin, ...added];
}

/**
 * Keeps only pages with at least one control in the processor's live
 * document. Statically derived pages cover options a unit may lack, such as
 * the MX structure, which is a paid upgrade on the 5700i, so they appear only
 * when the processor reports their fields. Without a document every page is kept.
 */
export function livePages(pages: LayoutPage[], fields: Record<string, unknown> | undefined): LayoutPage[] {
  if (!fields) return pages.filter(page => !page.when);
  return pages.filter(page => page.controls.some(control => control.name in fields)
    && (!page.when || (fields[page.when.field] as { value?: { value?: unknown } } | undefined)?.value?.value === page.when.value)
    && structureOnAir(page, fields));
}

/** Pages of the original clients that belong to the 5-band structure. */
const FIVE_BAND_PAGES = new Set(['Multiband', 'Compressors', 'Compressors (B4/5 Linked)', 'Band Mix',
  'Distortion', 'Distortion Control', 'MX Distortion Control', 'Speech Mode', 'MX Speech Mode']);

/** The structure a page belongs to, or null for a page every structure has. */
export function pageStructure(page: LayoutPage): '2band' | '5band' | null {
  if (page.title.includes('2 Band')) return '2band';
  return FIVE_BAND_PAGES.has(page.title) ? '5band' : null;
}

/**
 * Whether a page's processing structure is on air, read from the on-air
 * document as the meters do: a 5-band document carries `B1 ATTACK`, a 2-band
 * one `2B DRIVE`, and HD has its own fields. Several 2-band pages share a
 * field or two with the 5-band document (`MB GATE THR`, `SPEECH THR`), so a
 * single shared control must not bring them in.
 */
export function structureOnAir(page: LayoutPage, fields: Record<string, unknown>): boolean {
  const structure = pageStructure(page);
  if (!structure) return true;
  const prefix = page.path === 'HD' ? 'HD ' : '';
  return (structure === '5band' ? `${prefix}B1 ATTACK` : `${prefix}2B DRIVE`) in fields;
}

/**
 * The controls a page shows on this processor: only those in its on-air
 * document, with the page's on/off switch first, as in the Figma design.
 */
export function pageControls<T extends { name: string; kind: string }>(controls: T[], fields: Record<string, unknown> | undefined,
  isSwitch: (control: T) => boolean): T[] {
  const present = fields ? controls.filter(control => control.name in fields) : controls;
  const first = present.findIndex(isSwitch);
  return first > 0 ? [present[first], ...present.slice(0, first), ...present.slice(first + 1)] : present;
}

/** Short tab names in the style of the Figma tab bar; the full title stays as tooltip. */
const TAB_LABELS: Record<string, string> = {
  'Equalizer': 'EQ',
  'Speech Mode': 'Speech mode',
  'MX Speech Mode': 'MX Speech',
  'Distortion Control': 'Distortion',
  'MX Distortion Control': 'MX Distortion',
  'Compressors (B4/5 Linked)': 'B4/5 Linked',
};

export function tabLabel(title: string): string {
  return TAB_LABELS[title] ?? title;
}
