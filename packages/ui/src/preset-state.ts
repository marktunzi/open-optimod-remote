import type { Snapshot } from './model';

export function modifiedFieldNames(snapshot: Pick<Snapshot, 'modified_fields'>): Set<string> {
  return new Set(snapshot.modified_fields || []);
}

export function optimisticModifiedFields(current: string[] | undefined, fieldName: string): string[] {
  return Array.from(new Set([...(current || []), fieldName])).sort();
}

export function optimisticLessMoreAvailability(current: boolean | undefined, fieldName: string): boolean {
  return fieldName === 'LESS MORE' ? current !== false : false;
}
