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
