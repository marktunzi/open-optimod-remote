import { test } from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { MeterMotion, REFERENCE_METERS, meterModelFromProfile, visibleGroups } from '../src/meters.ts';
import { groupProcessingPage, type LayoutPage } from '../src/processing-layout.ts';
import { processingPages } from '../src/navigation-model.ts';

const profile = (path: string) => JSON.parse(readFileSync(new URL(`../../../profiles/${path}`, import.meta.url), 'utf8'));
const MODELS = ['5500/1.2.8.24', '8700hd/1.0.2.161'];

test('profile meters build their own groups and accept variable record lengths', () => {
  const m5500 = meterModelFromProfile(profile('5500/1.2.8.24/meters.json'));
  assert.deepEqual(m5500.groups.find(g => g.name === 'Input')!.channels.map(c => c.ids), [[1, 2]]);
  assert.deepEqual(m5500.groups.find(g => g.name === 'Gain Reduction')!.channels.map(c => c.ids), [[5], [6], [7], [8], [9]]);
  const motion = new MeterMotion(m5500);
  motion.push(Array(79).fill(50), 0);
  assert.ok(motion.sample(0, true));
  // The reference 5700i model still requires its exact 112-value records.
  const reference = new MeterMotion(REFERENCE_METERS);
  reference.push(Array(79).fill(50), 0);
  assert.equal(reference.sample(0, true), null);
  // Curves come from the profile; channels without one are linear.
  assert.ok(m5500.percent(1, 100) > 90);
  assert.equal(m5500.percent(5, 40), 40);
});

test('8700HD meter view hides HD groups in FM and composite in HD', () => {
  const model = meterModelFromProfile(profile('8700hd/1.0.2.161/meters.json'));
  assert.ok(model.groups.some(g => g.name === 'HD Gain Reduction'));
  assert.ok(visibleGroups('FM', model).every(g => !g.name.startsWith('HD ')));
  assert.ok(!visibleGroups('HD', model).some(g => g.kind === 'composite'));
  assert.deepEqual(model.groups.find(g => g.name === 'HD Limiting')!.channels.flatMap(c => c.ids), [19, 20]);
});

test('profile pages have unique titles per path and group without losing controls', () => {
  for (const path of MODELS) {
    const pages: LayoutPage[] = profile(`${path}/layouts.json`);
    for (const view of ['FM', 'HD'] as const) {
      const shown = processingPages(pages, view, 'Indepen.');
      const titles = shown.map(page => page.title);
      assert.equal(new Set(titles).size, titles.length, `${path} ${view}: ${titles}`);
    }
    for (const page of pages) {
      const grouped = groupProcessingPage(page).flatMap(group => group.controls.map(control => control.name));
      assert.deepEqual(new Set(grouped), new Set(page.controls.map(control => control.name)), `${path} ${page.title}`);
    }
  }
});

test('5700i keeps its built-in pages and gains only new structures from the supplement', async () => {
  const { mergeSupplementPages } = await import('../src/navigation-model.ts');
  const builtin: LayoutPage[] = JSON.parse(readFileSync(new URL('../src/layouts.json', import.meta.url), 'utf8'));
  const supplement: LayoutPage[] = profile('5700i/3.0.1.20/static-layouts.json');
  const merged = mergeSupplementPages(builtin, supplement);
  assert.deepEqual(merged.slice(0, builtin.length), builtin);
  const added = merged.slice(builtin.length).map(page => `${page.path} ${page.title}`);
  assert.ok(added.includes('FM 2 Band'));
  assert.ok(added.includes('HD 2 Band'));
  assert.ok(!added.includes('FM Distortion Control'), 'duplicates the built-in Distortion page');
  for (const view of ['FM', 'HD'] as const) {
    const titles = processingPages(merged, view, 'Indepen.').map(page => page.title);
    assert.equal(new Set(titles).size, titles.length, titles.join(', '));
  }
});

test('statically derived pages appear only when the processor reports their fields', async () => {
  const { livePages } = await import('../src/navigation-model.ts');
  const supplement: LayoutPage[] = profile('5700i/3.0.1.20/static-layouts.json');
  // A 5-band preset on a 5700i without the MX upgrade reports neither 2B nor MX fields.
  const fiveBand = livePages(supplement, { 'B1 COMP THRSH': {}, 'LESS MORE': {} }).map(page => page.title);
  assert.ok(!fiveBand.some(title => title.startsWith('MX') || title.includes('2 Band')), fiveBand.join(', '));
  const mx = livePages(supplement, { 'MX CLIP DRIVE': {} }).map(page => page.title);
  assert.ok(mx.includes('MX Distortion Control'), mx.join(', '));
  assert.equal(livePages(supplement, undefined).length, supplement.length);
});
