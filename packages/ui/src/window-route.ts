import type { Snapshot } from './model';

/**
 * The app's top-level windows. The macOS app opens each one as its own native
 * window on one processor; the browser keeps them together in one page
 * ('combined').
 */
export const WINDOW_KINDS = ['meters', 'processing', 'presets'] as const;
export type WindowKind = (typeof WINDOW_KINDS)[number];

export type WindowRoute = {
  kind: WindowKind | 'combined';
  /** The processor this window belongs to; null follows whichever is connected. */
  deviceId: string | null;
};

export function parseWindowRoute(search: string): WindowRoute {
  const query = new URLSearchParams(search);
  const kind = query.get('window');
  const deviceId = query.get('device');
  return {
    kind: (WINDOW_KINDS as readonly string[]).includes(kind ?? '') ? kind as WindowKind : 'combined',
    deviceId: deviceId ? deviceId : null,
  };
}

/**
 * The stable identity of the connected processor: its saved connection. A
 * connection made without one is named by its address, never by a port.
 */
export function deviceIdentity(snapshot: Pick<Snapshot, 'device_id' | 'host'>): string | null {
  if (snapshot.device_id) return snapshot.device_id;
  return snapshot.host ? `host:${snapshot.host}` : null;
}

/**
 * How a window relates to the session: showing its own processor, waiting for
 * it, or holding back the data of a different processor that is connected.
 */
export type DeviceBinding = 'own' | 'offline' | 'other-device';

export function deviceBinding(route: WindowRoute, snapshot: Pick<Snapshot, 'connected' | 'device_id' | 'host'>): DeviceBinding {
  if (!snapshot.connected) return 'offline';
  if (route.deviceId === null) return 'own';
  return deviceIdentity(snapshot) === route.deviceId ? 'own' : 'other-device';
}

/**
 * The snapshot a window may show. A window bound to one processor never shows
 * another processor's settings, presets or errors.
 */
export function scopedSnapshot<T extends Pick<Snapshot, 'connected' | 'device_id' | 'host'>>(route: WindowRoute, snapshot: T, offline: T): T {
  return deviceBinding(route, snapshot) === 'other-device' ? offline : snapshot;
}
