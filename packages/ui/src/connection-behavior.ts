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
