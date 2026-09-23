import { test } from 'node:test';
import assert from 'node:assert/strict';
import {
  filterConnections,
  maskedCode,
  nextSelectionAfterDelete,
  shouldDisconnectBeforeDelete,
  type ConnectionSummary,
} from '../src/connection-behavior.ts';

const devices: ConnectionSummary[] = [
  { id: 'a', name: 'Main studio', host: '192.0.2.115' },
  { id: 'b', name: 'Reserve', host: '10.0.0.22' },
  { id: 'c', name: 'Mobile rack', host: '172.16.0.7' },
];

test('only deleting the connected device requires a disconnect first', () => {
  assert.equal(shouldDisconnectBeforeDelete(true, 'a', 'a'), true);
  assert.equal(shouldDisconnectBeforeDelete(true, 'a', 'b'), false);
  assert.equal(shouldDisconnectBeforeDelete(false, 'a', 'a'), false);
});

test('connection search matches names and addresses without case sensitivity', () => {
  assert.deepEqual(filterConnections(devices, 'STUDIO').map(device => device.id), ['a']);
  assert.deepEqual(filterConnections(devices, '10.0').map(device => device.id), ['b']);
  assert.deepEqual(filterConnections(devices, '  ').map(device => device.id), ['a', 'b', 'c']);
});

test('deleting a connection selects its next neighbor and then its previous neighbor', () => {
  assert.equal(nextSelectionAfterDelete(devices, 'b'), 'c');
  assert.equal(nextSelectionAfterDelete(devices, 'c'), 'b');
  assert.equal(nextSelectionAfterDelete([{ id: 'a', name: 'Only', host: '127.0.0.1' }], 'a'), '');
});

test('saved codes render as bullets and unsaved codes remain empty', () => {
  assert.equal(maskedCode(true), '••••••••');
  assert.equal(maskedCode(false), '');
});
