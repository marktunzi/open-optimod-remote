import { test } from 'node:test';
import assert from 'node:assert/strict';
import { modifiedFieldNames, optimisticLessMoreAvailability, optimisticModifiedFields } from '../src/preset-state.ts';

test('preset modification state uses the processor-confirmed field list', () => {
  assert.equal(modifiedFieldNames({ modified_fields: ['AGC DRIVE'] }).has('AGC DRIVE'), true);
});

test('a locally adjusted control is marked immediately without duplicates', () => {
  assert.deepEqual(optimisticModifiedFields(['AGC DRIVE'], 'AGC DRIVE'), ['AGC DRIVE']);
  assert.deepEqual(optimisticModifiedFields(['AGC DRIVE'], 'MB DRIVE'), ['AGC DRIVE', 'MB DRIVE']);
});

test('advanced processing edits lock Less More while Less More itself remains available', () => {
  assert.equal(optimisticLessMoreAvailability(true, 'LESS MORE'), true);
  assert.equal(optimisticLessMoreAvailability(true, 'AGC DRIVE'), false);
  assert.equal(optimisticLessMoreAvailability(false, 'LESS MORE'), false);
});

test('on-device preset actions follow the capabilities and the on-air preset', async () => {
  const { presetManagement, presetNameProblem, maxPresetNameLength } = await import('../src/preset-state.ts');
  const base = {
    connected: true, host: 'h', firmware: '', processing: { name: 'MORNING', fields: {} }, system: null, error: null,
    revision: 1, presets: [{ name: 'ROCK', kind: 'Factory' as const }, { name: 'MORNING', kind: 'User' as const }, { name: 'EVENING', kind: 'User' as const }],
    write_enabled: true, modified_fields: [], adapter_id: 'pc-remote-5500-1.2.8.24',
    capabilities: { connect: true, parameter_reads: true, parameter_writes: true, live_meters: true, preset_catalog: true, preset_recall: true, preset_store: true, preset_delete: true },
  };
  assert.deepEqual(presetManagement(base, 'MORNING', false), { save: true, rename: true, delete: false });
  assert.deepEqual(presetManagement(base, 'EVENING', false), { save: true, rename: false, delete: true });
  assert.deepEqual(presetManagement(base, 'ROCK', false), { save: true, rename: false, delete: false });
  const modified = { ...base, processing: { name: 'modif MORNING', fields: {} } };
  assert.deepEqual(presetManagement(modified, 'MORNING', false), { save: true, rename: false, delete: false });
  const verified5700i = { ...base, capabilities: { ...base.capabilities, preset_store: false, preset_delete: false } };
  assert.deepEqual(presetManagement(verified5700i, 'EVENING', false), { save: false, rename: false, delete: false });
  assert.deepEqual(presetManagement(base, 'EVENING', true), { save: false, rename: false, delete: false });

  assert.equal(maxPresetNameLength('pc-remote-5500-1.2.8.24'), 18);
  assert.equal(maxPresetNameLength('pc-remote-5700i-3.0.1.20'), 20);
  assert.equal(presetNameProblem('NEW', base.presets, 18), null);
  assert.match(presetNameProblem('ROCK', base.presets, 18)!, /factory/);
  assert.match(presetNameProblem('EVENING', base.presets, 18)!, /already exists/);
  assert.match(presetNameProblem('NINETEEN CHARS 1234', base.presets, 18)!, /18/);
  assert.match(presetNameProblem('modif X', base.presets, 18)!, /modif/);
  assert.match(presetNameProblem('A[B', base.presets, 18)!, /brackets/);
});

test('preset files and backups are tied to the connected model', async () => {
  const { presetFileExtension, parseBackup } = await import('../src/preset-state.ts');
  assert.equal(presetFileExtension('pc-remote-5500-1.2.8.24'), 'orb55user');
  assert.equal(presetFileExtension('pc-remote-8700hd-1.0.2.161'), 'orb86user');
  assert.equal(presetFileExtension('pc-remote-5700i-3.0.1.20'), 'orb57user');
  assert.equal(presetFileExtension(null), 'orb');
  const backup = JSON.stringify({ format: 'open-optimod-backup', version: 1, created: 1, model: 'OPTIMOD 5500', adapter_id: 'pc-remote-5500-1.2.8.24', firmware: '1.2.8.24', processing: { name: 'A', document: 'x' }, system: { document: 'y' }, presets: [] });
  assert.equal(parseBackup(backup, 'pc-remote-5500-1.2.8.24').system.document, 'y');
  assert.throws(() => parseBackup(backup, 'pc-remote-5700i-3.0.1.20'), /same model and firmware/);
  assert.throws(() => parseBackup('{}', 'pc-remote-5500-1.2.8.24'), /not an Open Optimod Remote backup/);
  assert.throws(() => parseBackup('nonsense', 'pc-remote-5500-1.2.8.24'), /not an Open Optimod Remote backup/);
});
