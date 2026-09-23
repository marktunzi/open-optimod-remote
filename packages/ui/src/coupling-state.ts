import type { Field } from './model';

export type CouplingConfirmation = {
  field: Field;
  session: string;
  preset: string;
};

export function couplingConfirmationIsStale(
  confirmation: CouplingConfirmation | null,
  field: Field,
  session: string | null | undefined,
  preset: string,
): boolean {
  return !!confirmation && (
    confirmation.session !== session ||
    confirmation.preset !== preset ||
    JSON.stringify(confirmation.field) !== JSON.stringify(field)
  );
}
