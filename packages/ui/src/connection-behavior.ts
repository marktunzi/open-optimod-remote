export type ConnectionSummary = {
  id: string;
  name: string;
  host: string;
};

export function shouldDisconnectBeforeDelete(
  connected: boolean,
  activeId: string | null | undefined,
  selectedId: string,
): boolean {
  return connected && activeId === selectedId;
}

export function filterConnections<T extends ConnectionSummary>(devices: T[], query: string): T[] {
  const needle = query.trim().toLocaleLowerCase();
  if (!needle) return devices;
  return devices.filter(device =>
    `${device.name}\n${device.host}`.toLocaleLowerCase().includes(needle),
  );
}

export function nextSelectionAfterDelete(
  devices: ConnectionSummary[],
  deletedId: string,
): string {
  const index = devices.findIndex(device => device.id === deletedId);
  if (index < 0) return devices[0]?.id ?? '';
  return devices[index + 1]?.id ?? devices[index - 1]?.id ?? '';
}

export function maskedCode(hasCode: boolean): string {
  return hasCode ? '••••••••' : '';
}

export interface CodeIdentity {
  host: string;
  model?: string;
  has_code?: boolean;
}

/** A saved code survives an edit of the name or either port. A new address or
 *  processor model may be another device, so the service clears the code then
 *  and the editor must ask for it again. */
export function savedCodeSurvivesEdit(existing: CodeIdentity | undefined, edit: CodeIdentity): boolean {
  return Boolean(existing?.has_code)
    && existing!.host.trim() === edit.host.trim()
    && (existing!.model || 'auto') === (edit.model || 'auto');
}
