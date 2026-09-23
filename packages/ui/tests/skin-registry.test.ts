import { test } from 'node:test';
import assert from 'node:assert/strict';
import { PROCESSOR_SKINS, resolveProcessorSkin } from '../src/skin-registry.ts';

test('every supported processor has a distinct skin and product mark', () => {
  const skins = Object.values(PROCESSOR_SKINS);
  assert.equal(skins.length, 11);
  assert.equal(new Set(skins.map(skin => skin.id)).size, skins.length);
  assert.equal(new Set(skins.map(skin => skin.logo)).size, skins.length);
  assert.equal(new Set(skins.map(skin => skin.material)).size, skins.length);
  for (const skin of skins) {
    assert.match(skin.logo, /^\/assets\/optimod-/);
    assert.ok(skin.productName.startsWith('OPTIMOD '));
  }
});

test('a missing or unknown detected skin uses a neutral read-only appearance', () => {
  assert.equal(resolveProcessorSkin(undefined).id, 'neutral');
  assert.equal(resolveProcessorSkin('future-model').id, 'neutral');
  assert.equal(resolveProcessorSkin('optimod-5500i').id, 'optimod-5500i');
  assert.equal(resolveProcessorSkin('optimod-8700i').id, 'optimod-8700i');
});
