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
