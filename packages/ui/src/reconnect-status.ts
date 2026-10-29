export interface ReconnectStatus {
  attempt: number;
  attempts: number;
  next_attempt_at_ms: number;
  in_progress: boolean;
  reason: string;
}

/** Remaining wait as m:ss, never negative. */
export function remaining(atMs: number, nowMs: number): string {
  const seconds = Math.max(0, Math.ceil((atMs - nowMs) / 1000));
  return `${Math.floor(seconds / 60)}:${String(seconds % 60).padStart(2, '0')}`;
}

/** The banner text while the app waits to reconnect after a lost session. */
export function reconnectMessage(status: ReconnectStatus, nowMs: number): string {
  const attempt = `attempt ${status.attempt} of ${status.attempts}`;
  const next = status.in_progress
    ? `Reconnecting now (${attempt})…`
    : `Reconnecting in ${remaining(status.next_attempt_at_ms, nowMs)} (${attempt}).`;
  return `Connection lost: ${status.reason}. ${next} Controls return after the processor has been read again.`;
}
