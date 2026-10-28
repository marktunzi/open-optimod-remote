import { test } from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { MeterMotion, REFERENCE_METERS, meterModelFromProfile, visibleGroups } from '../src/meters.ts';
import { groupProcessingPage, type LayoutPage } from '../src/processing-layout.ts';
import { livePages, processingPages } from '../src/navigation-model.ts';

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

test('2-band and 5-band meter groups follow the on-air document of every model', () => {
  const fm5 = { 'Processing:B1 ATTACK': '20', 'Processing:HD B1 ATTACK': '20' };
  const fm2 = { 'Processing:2B DRIVE': '0', 'Processing:HD 2B DRIVE': '0' };
  const expected: Record<string, [string[], string[]]> = {
    '5500i/3.1.0.2': [['Gain Reduction'], ['2-Band Gain Reduction', 'HF Limiting', 'Overshoot Limit']],
    '6300/4.1.1.19': [['Gain Reduction'], ['2-Band Gain Reduction']],
    '8700hd/1.0.2.161': [['Gain Reduction', 'HD Gain Reduction'], ['2-Band Gain Reduction', 'HF Limiting', '2-Band HD Gain Reduction']],
    '8700i/1.5.1.11': [['Gain Reduction', 'HD Gain Reduction'], ['2-Band Gain Reduction', 'HF Limiting', '2-Band HD Gain Reduction']],
  };
  for (const [version, [five, two]] of Object.entries(expected)) {
    const model = meterModelFromProfile(profile(`${version}/meters.json`));
    const shown = (values: Record<string, string>) => visibleGroups('Both', model, values).map(g => g.name);
    for (const name of two) assert.ok(!shown(fm5).includes(name), `${version} 5-band shows ${name}`);
    for (const name of five) assert.ok(shown(fm5).includes(name), `${version} 5-band hides ${name}`);
    for (const name of five) assert.ok(!shown(fm2).includes(name), `${version} 2-band shows ${name}`);
    for (const name of two) assert.ok(shown(fm2).includes(name), `${version} 2-band hides ${name}`);
    // The 2-band groups take the place of the 5-band ones.
    assert.equal(shown(fm2).indexOf(two[0]), shown(fm5).indexOf(five[0]), version);
  }
});

test('the 8400 meters use the unit\'s own bar heights and scales', () => {
  const model = meterModelFromProfile(profile('8400/3.0.5.7/meters.json'));
  assert.deepEqual(model.groups.map(g => g.name), ['Input', 'AGC', 'Multiband', '2 Band', 'HF', 'Out', 'PWR', 'Comp']);
  const shown = (structure?: string) => visibleGroups('Both', model, structure ? {'Processing:2B/5B SWITCH': structure} : {}).map(g => g.name);
  assert.deepEqual(shown('5band'), ['Input', 'AGC', 'Multiband', 'Out', 'Comp']);
  assert.deepEqual(shown('2band'), ['Input', 'AGC', '2 Band', 'HF', 'Out', 'PWR', 'Comp']);
  assert.deepEqual(shown(), ['Input', 'AGC', 'Out', 'Comp']);
  const input = model.groups[0];
  assert.deepEqual(input.ticks?.[0], [0, '0']);
  assert.deepEqual(input.ticks?.at(-1), [1, '40']);
  // -10 dBFS sits at half height on the unit's screen.
  assert.ok(input.ticks?.some(([position, label]) => label === '10' && position === 0.5));
  assert.equal(model.percent(1, 42), 42);
  assert.equal(model.groups.find(g => g.name === 'Out')?.ticks?.[1]?.[1], '0');
});

test('the 8400 shows Less-More and the Advanced Modify pages of the structure on air', () => {
  const layouts = profile('8400/3.0.5.7/layouts.json') as LayoutPage[];
  const field = (value: string) => ({ value: { type: 'Choice', value }, index: 0 });
  const fields = (structure: string) => ({ 'LESS MORE': field('x'), 'SE AMOUNT': field('x'), 'AGC': field('On'), 'B1 COMP THRSH': field('x'), '2B DRIVE': field('x'), 'AGC XOVER': field('x'), 'B1 MIX': field('x'), 'BASS CLIP THR': field('x'), 'BASS GAIN': field('x'), 'MB DRIVE': field('x'), 'B1 ATTACK': field('x'), '2B/5B SWITCH': field(structure) });
  const titles = (structure: string) => processingPages(livePages(layouts, fields(structure)), 'FM', undefined).map(page => page.title);
  for (const structure of ['5band', '2band']) {
    const shown = titles(structure);
    assert.equal(shown[0], 'Less More');
    assert.ok(shown.slice(1).every(title => title.startsWith('Advanced Modify')), shown.join());
    assert.equal(new Set(shown).size, shown.length, 'one variant per page');
  }
  const two = processingPages(livePages(layouts, fields('2band')), 'FM', undefined);
  assert.ok(two.find(page => page.title === 'Advanced Modify 2')?.controls.some(control => control.name === 'AGC XOVER'));
});
