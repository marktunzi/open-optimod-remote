import { test } from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';

const source = readFileSync(new URL('../src/main.tsx', import.meta.url), 'utf8');

test('routine writes do not render confirmation or waiting banners between meters and controls', () => {
  assert.doesNotMatch(source, /workspace-status/);
  assert.doesNotMatch(source, /confirmed by the Optimod/);
  assert.doesNotMatch(source, /Waiting for the processor/);
});
