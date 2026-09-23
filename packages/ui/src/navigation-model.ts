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
