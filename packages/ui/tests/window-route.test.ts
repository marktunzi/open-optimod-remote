import { test } from 'node:test';
import assert from 'node:assert/strict';
import { deviceBinding, deviceIdentity, parseWindowRoute, scopedSnapshot } from '../src/window-route.ts';

test('a native window names its kind and processor; the browser keeps one page', () => {
  assert.deepEqual(parseWindowRoute('?window=meters&device=abc'), { kind: 'meters', deviceId: 'abc' });
  assert.deepEqual(parseWindowRoute('?window=processing'), { kind: 'processing', deviceId: null });
  assert.deepEqual(parseWindowRoute('?window=system-settings&device=x'), { kind: 'combined', deviceId: 'x' });
  assert.deepEqual(parseWindowRoute(''), { kind: 'combined', deviceId: null });
  assert.deepEqual(parseWindowRoute('?window=unknown&device='), { kind: 'combined', deviceId: null });
});

test('a processor is identified by its saved connection, else its address, never its port', () => {
  assert.equal(deviceIdentity({ device_id: 'd1', host: '192.168.1.171' }), 'd1');
  assert.equal(deviceIdentity({ device_id: null, host: '192.168.1.171' }), 'host:192.168.1.171');
  assert.equal(deviceIdentity({ device_id: null, host: '' }), null);
});

test('a window bound to one processor holds back another processor\'s data', () => {
  const own = { connected: true, device_id: 'd1', host: '192.168.1.177', marker: 'own' };
  const other = { connected: true, device_id: 'd2', host: '192.168.1.171', marker: 'other' };
  const offline = { connected: false, device_id: null, host: '', marker: 'offline' };
  const route = { kind: 'meters' as const, deviceId: 'd1' };
  assert.equal(deviceBinding(route, own), 'own');
  assert.equal(deviceBinding(route, other), 'other-device');
  assert.equal(deviceBinding(route, offline), 'offline');
  assert.equal(scopedSnapshot(route, own, offline).marker, 'own');
  assert.equal(scopedSnapshot(route, other, offline).marker, 'offline');
  assert.equal(deviceBinding({ kind: 'combined', deviceId: null }, other), 'own');
});

test('a window of one processor talks only to that processor session', async () => {
  const { apiPath } = await import('../src/api.ts');
  assert.equal(apiPath('state', '?window=meters&device=4f1c'), '/api/d/4f1c/state');
  assert.equal(apiPath('meters', '?window=meters&device=host%3A192.168.1.171'), '/api/d/host%3A192.168.1.171/meters');
  // The browser page keeps the single unprefixed session.
  assert.equal(apiPath('state', ''), '/api/state');
});
