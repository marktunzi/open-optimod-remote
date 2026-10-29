import { test } from 'node:test';
import assert from 'node:assert/strict';
import { reconnectMessage, remaining, type ReconnectStatus } from '../src/reconnect-status.ts';

const waiting: ReconnectStatus = {
  attempt: 1,
  attempts: 6,
  next_attempt_at_ms: 1_000_000 + 120_000,
  in_progress: false,
  reason: 'Device disconnected',
};

test('the wait counts down in minutes and seconds and never goes negative', () => {
  assert.equal(remaining(waiting.next_attempt_at_ms, 1_000_000), '2:00');
  assert.equal(remaining(waiting.next_attempt_at_ms, 1_000_000 + 60_500), '1:00');
  assert.equal(remaining(waiting.next_attempt_at_ms, 1_000_000 + 119_001), '0:01');
  assert.equal(remaining(waiting.next_attempt_at_ms, 1_000_000 + 200_000), '0:00');
});

test('the banner names the reason, the next attempt and the attempt count', () => {
  assert.equal(
    reconnectMessage(waiting, 1_000_000),
    'Connection lost: Device disconnected. Reconnecting in 2:00 (attempt 1 of 6). Controls return after the processor has been read again.',
  );
  assert.match(reconnectMessage({ ...waiting, in_progress: true }, 1_000_000), /Reconnecting now \(attempt 1 of 6\)/);
});
