import { test } from 'node:test';
import assert from 'node:assert/strict';
import {
  DEFAULT_METER_VIEW,
  TOP_WORKSPACES,
  defaultWorkspace,
  initialWorkspace,
  processingControlEditable,
  processingPages,
  processingPathSelectorVisible,
  systemSettingsTarget,
} from '../src/navigation-model.ts';
import type { LayoutPage } from '../src/processing-layout.ts';

test('connections, processing and presets are first-class workspaces', () => {
  assert.deepEqual(TOP_WORKSPACES, ['Processing', 'Presets', 'Connections']);
  assert.equal(defaultWorkspace(false), 'Connections');
  assert.equal(defaultWorkspace(true), 'Processing');
});

test('the macOS app delegates connection management to its native window', () => {
  assert.equal(initialWorkspace(false, true), 'Processing');
  assert.equal(initialWorkspace(false, false), 'Connections');
  assert.equal(initialWorkspace(true, true), 'Processing');
});

test('the instrument always shows both FM and HD meter groups', () => {
  assert.equal(DEFAULT_METER_VIEW, 'Both');
});

test('the FM and HD path selector only appears while processing is decoupled', () => {
  assert.equal(processingPathSelectorVisible('FM->HD'), false);
  assert.equal(processingPathSelectorVisible('Indepen.'), true);
  assert.equal(processingPathSelectorVisible(undefined), false);
});

test('coupled processing keeps the unique HD limiting page available', () => {
  const layouts = [
    { title: 'AGC', path: 'Shared' },
    { title: 'Equalizer', path: 'FM' },
    { title: 'Equalizer', path: 'HD' },
    { title: 'HD Limiting', path: 'HD' },
  ] as LayoutPage[];

  assert.deepEqual(
    processingPages(layouts, 'FM', 'FM->HD').map(page => `${page.path}:${page.title}`),
    ['Shared:AGC', 'FM:Equalizer', 'HD:HD Limiting'],
  );
});

test('only HD Limiting controls remain editable while FM to HD is coupled', () => {
  for (const name of ['IBOC EQ GAIN', 'IBOC EQ FREQ', 'IBOC LIM DR', 'HD DE ESS', 'HD COUPLING']) {
    assert.equal(processingControlEditable(name, 'FM->HD'), true, name);
  }
  for (const name of ['HD PEQ LOW GAIN', 'HD MB DRIVE', 'HD B1 COMP THRSH', 'HD B1 OUTPUT MIX']) {
    assert.equal(processingControlEditable(name, 'FM->HD'), false, name);
  }
  assert.equal(processingControlEditable('HD MB DRIVE', 'Indepen.'), true);
  assert.equal(processingControlEditable('FINAL CLIP DRV', 'FM->HD'), true);
});

test('setup uses the native settings window when the macOS bridge exists', () => {
  assert.equal(systemSettingsTarget(true), 'native');
  assert.equal(systemSettingsTarget(false), 'workspace');
});
