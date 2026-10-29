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
 * when the processor reports their fields. Without a live document no page
 * is kept: which structure (2-band or 5-band) is on air is then unknown.
 */
export function livePages(pages: LayoutPage[], fields: Record<string, unknown> | undefined): LayoutPage[] {
  if (!fields) return [];
  const mx = mxOnAir(fields);
  const titles = new Set(pages.map(page => `${page.path}:${page.title}`));
  return pages.filter(page => page.controls.some(control => control.name in fields)
    && (!page.when || (fields[page.when.field] as { value?: { value?: unknown } } | undefined)?.value?.value === page.when.value)
    && structureOnAir(page, fields)
    && mxPageOnAir(page, mx, titles)
    && (!(page.title in PAGE_FIELD) || PAGE_FIELD[page.title] in fields));
}

/**
 * The processing pages a device window shows: the model's own layouts, or
 * the 5700i's built-in pages with its statically derived supplement, kept
 * to the structure in the live on-air document. Without one the structure
 * (2-band or 5-band) is unknown, so no page is shown.
 */
export function shownProcessingPages(modelLayouts: LayoutPage[] | null | undefined, builtin: LayoutPage[],
  supplement: LayoutPage[], fields: Record<string, unknown> | undefined, path: 'FM' | 'HD', coupling: string | undefined): LayoutPage[] {
  if (!fields) return [];
  return processingPages(
    modelLayouts
      ? livePages(modelLayouts, fields)
      : mergeSupplementPages(builtin.filter(page => structureOnAir(page, fields)), livePages(supplement, fields)),
    path,
    coupling,
  );
}

/**
 * Pages that share most controls with another page and appear only with the
 * field that sets them apart: the original 8700HD client shows one
 * Compressors tab in every 5-band preset whose document lacks `B4/5 DELTA REL`.
 */
const PAGE_FIELD: Record<string, string> = {
  'Compressors (B4/5 Linked)': 'B4/5 DELTA REL',
};

/**
 * MX pages and the pages they replace, as the original clients switch them
 * (8700HD PC Remote 1.0.2.161, 28 October 2026): with `MX SWITCH` 1 the
 * Distortion, Speech Mode and Final Clipping tabs show the MX dialogs; with
 * `MX SWITCH` 0 or absent they show the standard ones, even when the
 * document carries the MX fields (a 5700i without the MX upgrade does).
 */
const MX_REPLACES: Record<string, string[]> = {
  // The 5700i's built-in page is called Distortion.
  'MX Distortion Control': ['Distortion Control', 'Distortion'],
  'MX 2 Band Distortion': ['2 Band Distortion'],
  'MX Speech Mode': ['Speech Mode'],
  'Final Clip': ['Final Clipping'],
};
const MX_REPLACED = new Map(Object.entries(MX_REPLACES).flatMap(([mx, standards]) => standards.map(standard => [standard, mx] as const)));

export function mxStructure(page: LayoutPage): 'mx' | 'standard' | null {
  if (page.path !== 'FM') return null;
  if (page.title in MX_REPLACES) return 'mx';
  return MX_REPLACED.has(page.title) ? 'standard' : null;
}

/** Whether the MX structure is on air: the document's `MX SWITCH` is 1. */
export function mxOnAir(fields: Record<string, unknown>): boolean {
  const value = (fields['MX SWITCH'] as { value?: { value?: unknown } } | undefined)?.value?.value;
  return value === 1 || value === '1' || value === 'On';
}

function mxPageOnAir(page: LayoutPage, mx: boolean, titles: Set<string>): boolean {
  const kind = mxStructure(page);
  if (kind === 'mx') return mx;
  // A standard page gives way only to an MX page this model actually has.
  if (kind === 'standard') return !mx || !titles.has(`FM:${MX_REPLACED.get(page.title)}`);
  return true;
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

/**
 * Short tab names in the style of the Figma tab bar; the full title stays as
 * tooltip. An MX page takes the tab name of the page it replaces, as in the
 * original client.
 */
const TAB_LABELS: Record<string, string> = {
  'Equalizer': 'EQ',
  'Speech Mode': 'Speech mode',
  'MX Speech Mode': 'Speech mode',
  'Distortion Control': 'Distortion',
  'MX Distortion Control': 'Distortion',
  '2 Band Distortion': '2B Distortion',
  'MX 2 Band Distortion': 'Distortion',
  'Final Clip': 'Final Clipping',
  'Compressors (B4/5 Linked)': 'B4/5 Linked',
};

export function tabLabel(title: string): string {
  return TAB_LABELS[title] ?? title;
}
