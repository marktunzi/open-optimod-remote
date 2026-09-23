import { test } from 'node:test';
import assert from 'node:assert/strict';
import { nextControlIndex } from '../src/control-keyboard.ts';

test('value fields support arrow, page, home and end keys', () => {
  assert.equal(nextControlIndex(4, 9, 'ArrowUp'), 5);
  assert.equal(nextControlIndex(4, 9, 'ArrowLeft'), 3);
  assert.equal(nextControlIndex(4, 9, 'PageUp'), 9);
  assert.equal(nextControlIndex(4, 20, 'PageDown'), 0);
  assert.equal(nextControlIndex(4, 9, 'Home'), 0);
  assert.equal(nextControlIndex(4, 9, 'End'), 9);
  assert.equal(nextControlIndex(0, 9, 'ArrowDown'), 0);
  assert.equal(nextControlIndex(9, 9, '+'), 9);
  assert.equal(nextControlIndex(4, 9, 'Tab'), null);
});
