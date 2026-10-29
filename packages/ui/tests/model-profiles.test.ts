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
  assert.ok(motion.sample(0));
  // The reference 5700i model still requires its exact 112-value records.
  const reference = new MeterMotion(REFERENCE_METERS);
  reference.push(Array(79).fill(50), 0);
  assert.equal(reference.sample(0), null);
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

test('2-band and 5-band processing pages follow the on-air document, even with shared fields', async () => {
  const { livePages, pageControls } = await import('../src/navigation-model.ts');
  const layouts: LayoutPage[] = profile('8700hd/1.0.2.161/layouts.json');
  const fieldsOf = (names: string[]) => Object.fromEntries(names.map(name => [name, {}]));
  const all = (titles: string[]) => layouts.filter(page => titles.includes(page.title)).flatMap(page => page.controls.map(c => c.name));
  // A 5-band document shares MB GATE THR and SPEECH THR with the 2-band pages.
  const five = fieldsOf([...all(['Less More', 'AGC', 'Multiband', 'Compressors', 'Band Mix', 'Speech Mode', 'Distortion Control']), 'B1 ATTACK', 'HD B1 ATTACK']);
  const two = fieldsOf([...all(['Less More', 'AGC', '2 Band', '2 Band Distortion']), 'SPEECH THR', '2B DRIVE', 'HD 2B DRIVE']);
  const titles = (fields: Record<string, unknown>) => livePages(layouts, fields).map(page => `${page.path}:${page.title}`);
  assert.ok(!titles(five).some(title => title.includes('2 Band')), titles(five).join(', '));
  assert.ok(titles(five).includes('FM:Multiband') && titles(five).includes('FM:Speech Mode'));
  for (const title of ['FM:Multiband', 'FM:Compressors', 'FM:Band Mix', 'FM:Speech Mode', 'FM:Distortion Control'])
    assert.ok(!titles(two).includes(title), `${title} in 2-band`);
  assert.ok(titles(two).includes('FM:2 Band') && titles(two).includes('FM:2 Band Distortion'));
  // Only controls in the document, the page's switch first.
  const controls = [{ name: 'AGC DRIVE', kind: 'slider' }, { name: 'AGC', kind: 'radio' }, { name: 'AGC MISSING', kind: 'slider' }];
  assert.deepEqual(pageControls(controls, { 'AGC DRIVE': {}, 'AGC': {} }, c => c.kind === 'radio').map(c => c.name), ['AGC', 'AGC DRIVE']);
  assert.deepEqual(pageControls(controls, undefined, () => false).map(c => c.name), ['AGC DRIVE', 'AGC', 'AGC MISSING']);
});

test('statically derived pages appear only when the processor reports their fields', async () => {
  const { livePages } = await import('../src/navigation-model.ts');
  const supplement: LayoutPage[] = profile('5700i/3.0.1.20/static-layouts.json');
  // A 5-band preset on a 5700i without the MX upgrade reports neither 2B nor MX fields.
  const fiveBand = livePages(supplement, { 'B1 COMP THRSH': {}, 'LESS MORE': {} }).map(page => page.title);
  assert.ok(!fiveBand.some(title => title.startsWith('MX') || title.includes('2 Band')), fiveBand.join(', '));
  const mx = livePages(supplement, { 'MX CLIP DRIVE': {}, 'B1 ATTACK': {}, 'MX SWITCH': { index: 1, value: { type: 'Int', value: 1 } } }).map(page => page.title);
  assert.ok(mx.includes('MX Distortion Control'), mx.join(', '));
  // MX fields with MX SWITCH 0 (a 5700i without the upgrade) show no MX page.
  const off = livePages(supplement, { 'MX CLIP DRIVE': {}, 'B1 ATTACK': {}, 'MX SWITCH': { index: 0, value: { type: 'Int', value: 0 } } }).map(page => page.title);
  assert.ok(!off.some(title => title.startsWith('MX') || title === 'Final Clip'), off.join(', '));
  assert.deepEqual(livePages(supplement, undefined), []);
});

test('an MX 5700i shows one Distortion, Speech Mode and Final Clipping tab', async () => {
  const { livePages, mergeSupplementPages, processingPages, tabLabel } = await import('../src/navigation-model.ts');
  const builtin = (await import('../src/layouts.json', { with: { type: 'json' } })).default as LayoutPage[];
  const merged = mergeSupplementPages(builtin, profile('5700i/3.0.1.20/static-layouts.json'));
  const names = merged.filter(page => page.path !== 'HD').flatMap(page => page.controls.map(c => c.name));
  const fields = (mx: number) => ({ ...Object.fromEntries(names.filter(n => !n.startsWith('2B')).map(n => [n, {}])), 'B1 ATTACK': {},
    'MX SWITCH': { index: mx, value: { type: 'Int', value: mx } } });
  for (const mx of [0, 1]) {
    const tabs = processingPages(livePages(merged, fields(mx)), 'FM', undefined).map(page => tabLabel(page.title));
    assert.equal(new Set(tabs).size, tabs.length, tabs.join(', '));
    for (const tab of ['Distortion', 'Speech mode', 'Final Clipping']) assert.equal(tabs.filter(t => t === tab).length, 1, `${mx}: ${tabs.join(', ')}`);
  }
});

test('MX pages replace the standard ones only while MX SWITCH is 1, as the original 8700HD client shows', async () => {
  const { livePages, tabLabel } = await import('../src/navigation-model.ts');
  const layouts: LayoutPage[] = profile('8700hd/1.0.2.161/layouts.json');
  const fieldsOf = (names: string[], mx?: number) => ({
    ...Object.fromEntries(names.map(name => [name, {}])),
    ...(mx === undefined ? {} : { 'MX SWITCH': { index: mx, value: { type: 'Int', value: mx } } }),
  });
  const all = (titles: string[]) => layouts.filter(page => page.path === 'FM' && titles.includes(page.title)).flatMap(page => page.controls.map(c => c.name));
  // Documents carry the fields of both variants (3FM STREAM V03 has every MX field with MX SWITCH 0).
  const five = all(['AGC', 'Multiband', 'Distortion Control', 'MX Distortion Control', 'Speech Mode', 'MX Speech Mode', 'Final Clipping', 'Final Clip']).concat('B1 ATTACK');
  const two = all(['AGC', '2 Band', '2 Band Distortion', 'MX 2 Band Distortion', 'Final Clipping', 'Final Clip']).concat('2B DRIVE');
  const tabs = (fields: Record<string, unknown>) => livePages(layouts, fields).filter(page => page.path === 'FM').map(page => tabLabel(page.title));
  const titles = (fields: Record<string, unknown>) => livePages(layouts, fields).filter(page => page.path === 'FM').map(page => page.title);
  // CLASSICAL-5 BAND / 3FM STREAM V03 (MX SWITCH 0) and GREGG MX (MX SWITCH 1)
  for (const mx of [undefined, 0]) {
    assert.deepEqual(titles(fieldsOf(five, mx)).filter(t => /Distortion|Speech|Final/.test(t)), ['Distortion Control', 'Final Clipping', 'Speech Mode']);
  }
  assert.deepEqual(titles(fieldsOf(five, 1)).filter(t => /Distortion|Speech|Final/.test(t)), ['MX Distortion Control', 'MX Speech Mode', 'Final Clip']);
  // CLASSICAL-2 BAND (2B Distortion) and CLASSICAL-2B+AGC MX (Distortion)
  assert.deepEqual(tabs(fieldsOf(two, 0)).filter(t => /Distortion|Speech|Final/.test(t)), ['2B Distortion', 'Final Clipping']);
  assert.deepEqual(tabs(fieldsOf(two, 1)).filter(t => /Distortion|Speech|Final/.test(t)), ['Distortion', 'Final Clipping']);
  assert.deepEqual(tabs(fieldsOf(five, 1)).filter(t => /Distortion|Speech|Final/.test(t)), ['Distortion', 'Speech mode', 'Final Clipping']);
  // One Compressors tab unless the document has the linked page's own field.
  const compressors = all(['Compressors', 'Compressors (B4/5 Linked)']).filter(n => n !== 'B4/5 DELTA REL');
  assert.deepEqual(titles(fieldsOf([...five, ...compressors], 0)).filter(t => t.startsWith('Compressors')), ['Compressors']);
  assert.ok(titles(fieldsOf([...five, ...compressors, 'B4/5 DELTA REL'], 0)).includes('Compressors (B4/5 Linked)'));
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

test('the 5700i 2-band view shows the 2-band gain reduction and HF limiter, as its original client does', () => {
  const two = visibleGroups('Both', REFERENCE_METERS, { 'Processing:2B DRIVE': '0', 'Processing:HD 2B DRIVE': '0' }).map(g => g.name);
  assert.deepEqual(two.filter(n => /2-Band|HF Limiting|Gain Reduction/.test(n)), ['FM 2-Band Gain Reduction', 'HF Limiting', 'HD 2-Band Gain Reduction']);
  const five = visibleGroups('Both', REFERENCE_METERS, { 'Processing:B1 ATTACK': '0', 'Processing:HD B1 ATTACK': '0' }).map(g => g.name);
  assert.ok(!five.includes('HF Limiting') && five.includes('FM Gain Reduction'));
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
  // The client's two GATE arrows: byte 25 (AGC) and 26 (multiband), lit on silence.
  assert.equal(model.groups.find(g => g.name === 'AGC')?.gate?.id, 25);
  assert.equal(model.groups.find(g => g.name === 'Multiband')?.gate?.id, 26);
});

test('the 8400 shows Less-More and the processing categories of the structure on air', () => {
  const layouts = profile('8400/3.0.5.7/layouts.json') as LayoutPage[];
  const field = (value: string) => ({ value: { type: 'Choice', value }, index: 0 });
  const shared = layouts.filter(page => page.path === 'Shared');
  const everything = Object.fromEntries(shared.flatMap(page => page.controls.map(control => [control.name, field('x')])));
  const fields = (structure: string) => ({ ...everything, '2B/5B SWITCH': field(structure) });
  const pages = (structure: string) => processingPages(livePages(layouts, fields(structure)), 'FM', undefined);
  assert.deepEqual(pages('5band').map(page => page.title),
    ['Less More', 'Stereo Enhancer', 'AGC', 'Equalizer', 'Multiband', 'Dynamics', 'Band Mix', 'Coupling', 'Clipping']);
  assert.deepEqual(pages('2band').map(page => page.title),
    ['Less More', 'Stereo Enhancer', 'AGC', 'Equalizer', 'Two Band', 'Clipping']);
  assert.ok(pages('2band').find(page => page.title === 'AGC')?.controls.some(control => control.name === 'AGC XOVER'));
  assert.ok(!pages('2band').some(page => page.controls.some(control => control.name === 'B1 OUTPUT MIX')));
  // Explicit sections, never the extracted x 0 coordinates: every control of the
  // structure's client pages appears exactly once, in a named section.
  const client = (structure: string) => new Set(shared
    .filter(page => !page.when || page.when.value === structure)
    .flatMap(page => page.controls.map(control => control.name)));
  for (const structure of ['5band', '2band']) {
    const placed = pages(structure).flatMap(page => groupProcessingPage(page).flatMap(group => {
      assert.ok(group.title && !/^Advanced Modify/.test(group.title), group.title);
      return group.controls.map(control => control.name);
    }));
    assert.equal(placed.length, new Set(placed).size, `${structure}: a control appears twice`);
    assert.deepEqual([...placed].sort(), [...client(structure)].sort());
  }
  for (const page of shared.filter(page => page.title !== 'Less More')) {
    assert.ok(page.sections?.length, `${page.title} has sections`);
  }
  assert.deepEqual(shared.filter(page => !page.when).map(page => page.title), ['Less More', 'Stereo Enhancer']);
});

test('issue #18: the 5700i shows no processing page until a live document names its structure', async () => {
  const { shownProcessingPages, pageStructure } = await import('../src/navigation-model.ts');
  const builtin = JSON.parse(readFileSync(new URL('../src/layouts.json', import.meta.url), 'utf8')) as LayoutPage[];
  const supplement: LayoutPage[] = profile('5700i/3.0.1.20/static-layouts.json');
  const titles = (fields: Record<string, unknown> | undefined) =>
    shownProcessingPages(null, builtin, supplement, fields, 'FM', 'Indepen.').map(page => page.title);
  // A failed sync has no document: neither 2-band nor 5-band pages, nor shared ones.
  assert.deepEqual(titles(undefined), []);
  // A document of one structure: every control of the shared pages and of
  // that structure's pages, plus the field that marks the structure.
  const document = (structure: '2band' | '5band', marker: string) => Object.fromEntries([...builtin, ...supplement]
    .filter(page => page.path !== 'HD' && !page.title.startsWith('MX') && page.title !== 'Final Clip'
      && (pageStructure(page) === null || pageStructure(page) === structure))
    .flatMap(page => page.controls.map(control => [control.name, {}]))
    .concat([[marker, {}]]));
  const fiveBand = titles(document('5band', 'B1 ATTACK'));
  assert.ok(['Multiband', 'Compressors', 'Band Mix'].every(title => fiveBand.includes(title)), fiveBand.join(', '));
  assert.ok(!fiveBand.some(title => title.includes('2 Band')), fiveBand.join(', '));
  const twoBand = titles(document('2band', '2B DRIVE'));
  assert.ok(['2 Band', '2 Band Distortion'].every(title => twoBand.includes(title)), twoBand.join(', '));
  assert.ok(!twoBand.some(title => pageStructure({ title } as LayoutPage) === '5band'), twoBand.join(', '));
});
