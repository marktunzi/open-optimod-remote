import { test } from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { binaryOnIndex, groupProcessingPage, type LayoutPage } from '../src/processing-layout.ts';

const root = fileURLToPath(new URL('../../..', import.meta.url));
const layouts = JSON.parse(readFileSync(`${root}/packages/ui/src/layouts.json`, 'utf8')) as LayoutPage[];
const profile = JSON.parse(readFileSync(`${root}/profiles/5700i/3.0.1.20/parameters.json`, 'utf8'));

test('Less More is the first shared page and binds the verified field', () => {
  assert.equal(layouts[0].title, 'Less More');
  assert.equal(layouts[0].controls[0].name, 'LESS MORE');
  assert.equal(layouts[0].controls[0].reference_id, 273);
  const field = profile.fields.find((candidate: { name: string }) => candidate.name === 'LESS MORE');
  assert.equal(field.reference_id, 273);
});

test('controls are assigned to named reference groups by their center point', () => {
  const page = layouts.find(candidate => candidate.path === 'FM' && candidate.title === 'Multiband')!;
  const groups = groupProcessingPage(page);
  assert.deepEqual(groups.map(group => group.title), ['Multiband', 'Multiband Coupling', 'Multiband Max Delta GR']);
  assert.deepEqual(groups.map(group => group.controls.length), [7, 5, 5]);
});

test('pages without named groups receive one useful page group', () => {
  const page = layouts.find(candidate => candidate.path === 'Shared' && candidate.title === 'AGC')!;
  const groups = groupProcessingPage(page);
  assert.equal(groups.length, 1);
  assert.equal(groups[0].title, 'AGC');
  assert.equal(groups[0].controls.length, page.controls.length);
});

test('Band Mix pairs each band level with its matching on off control', () => {
  const page = layouts.find(candidate => candidate.path === 'FM' && candidate.title === 'Band Mix')!;
  const group = groupProcessingPage(page)[0];
  assert.deepEqual(group.controls.map(control => control.name), [
    'B1 OUTPUT MIX', 'B1 ON/OFF',
    'B2 OUTPUT MIX', 'B2 ON/OFF',
    'B3 OUTPUT MIX', 'B3 ON/OFF',
    'B4 OUTPUT MIX', 'B4 ON/OFF',
    'B5 OUTPUT MIX', 'B5 ON/OFF',
  ]);
});

test('binary processing choices identify the affirmative toggle position', () => {
  assert.equal(binaryOnIndex(['Off', 'On']), 1);
  assert.equal(binaryOnIndex(['On', 'Off']), 0);
  assert.equal(binaryOnIndex(['Out', 'In']), 1);
  assert.equal(binaryOnIndex(['Slow', 'Fast']), 1);
  assert.equal(binaryOnIndex(['One', 'Two', 'Three']), null);
});

test('extracted pages read the original dialog column by column; group titles keep their acronyms', async () => {
  const { readingOrder } = await import('../src/processing-layout.ts');
  const control = (name: string, x: number, y: number) => ({ kind: 'slider', name, label: name, x, y, w: 105, id: 0, options: [] });
  const order = readingOrder([control('C', 182, 16), control('B', 13, 76), control('D', 182, 46), control('A', 13, 46)]).map(c => c.name);
  assert.deepEqual(order, ['A', 'B', 'C', 'D']);
  const page = { title: 'Equalizer', path: 'FM', width: 300, height: 200, reference_dialog: 393, groups: [{ x: 0, y: 0, w: 150, h: 200, text: ' LF SHELF EQUALIZER' }],
    controls: [control('BASS GAIN', 13, 46)] };
  assert.equal(groupProcessingPage(page)[0].title, 'LF Shelf Equalizer');
});
