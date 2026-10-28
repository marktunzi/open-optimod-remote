export type LayoutControl = {
  kind: string;
  name: string;
  label: string;
  x: number;
  y: number;
  w: number;
  id: number;
  options: string[];
  reference_id?: number;
  /** A text box's maximum length in the client (`kind: 'text'`). */
  max_length?: number;
};

export type LayoutGroup = {
  x: number;
  y: number;
  w: number;
  h: number;
  text: string;
};

export type LayoutPage = {
  title: string;
  path: string;
  width: number;
  height: number;
  reference_dialog: number;
  controls: LayoutControl[];
  groups: LayoutGroup[];
  /** Shown only while this processing field has this value (e.g. the 8400's 2B/5B SWITCH). */
  when?: { field: string; value: string };
};

export type ProcessingGroup = {
  title: string;
  controls: LayoutControl[];
  className?: string;
  order: number;
};

export function binaryOnIndex(options: string[]): number | null {
  if (options.length !== 2) return null;
  const affirmative = options.findIndex(option => /^(on|in|yes|enabled|operate)$/i.test(option.trim()));
  return affirmative >= 0 ? affirmative : 1;
}

const ACRONYMS = new Set(['RDS', 'LF', 'HF', 'MB', 'EQ', 'AGC', 'HD', 'FM', 'MX', 'GR', 'MPX', '2B', 'B4/5']);

function titleCase(value: string): string {
  return value
    .trim()
    .split(/\s+/)
    .map(word => ACRONYMS.has(word.toUpperCase()) ? word.toUpperCase()
      : word.toLocaleLowerCase().replace(/^\w/, character => character.toLocaleUpperCase()))
    .join(' ');
}

/**
 * The order a page's controls have on the original client's dialog: column
 * by column, top to bottom. Extracted pages list them in the order the
 * client binds them, which jumps around the dialog. A control starts a new
 * column when it sits more than 40 dialog units right of the column's first.
 */
export function readingOrder(controls: LayoutControl[]): LayoutControl[] {
  const byX = [...controls].sort((a, b) => a.x - b.x || a.y - b.y);
  const columns: LayoutControl[][] = [];
  for (const control of byX) {
    const column = columns.at(-1);
    if (column && control.x - column[0].x <= 40) column.push(control);
    else columns.push([control]);
  }
  return columns.flatMap(column => column.sort((a, b) => a.y - b.y));
}

function bandMixControls(controls: LayoutControl[]): LayoutControl[] {
  const result: LayoutControl[] = [];
  for (let band = 1; band <= 5; band += 1) {
    const token = `B${band} `;
    const matching = controls.filter(control => control.name.replace(/^HD /, '').startsWith(token));
    const mix = matching.find(control => control.name.includes('OUTPUT MIX'));
    const enabled = matching.find(control => control.name.includes('ON/OFF'));
    if (mix) result.push(mix);
    if (enabled) result.push(enabled);
  }
  return result;
}

export function groupProcessingPage(page: LayoutPage): ProcessingGroup[] {
  if (page.title === 'Band Mix') {
    return [{ title: 'Band Mix', controls: bandMixControls(page.controls), className: 'band-mix-group', order: 0 }];
  }

  const named = page.groups
    .filter(group => group.text.trim())
    .sort((a, b) => a.x - b.x || a.y - b.y);
  if (!named.length) {
    return [{ title: page.title, controls: page.controls, order: 0 }];
  }

  // A control belongs to the smallest group box around it, so a box inside
  // another (the 5700i's RDS Modulator in Preset RDS Settings) keeps its own.
  const inside = (control: LayoutControl, group: LayoutGroup) => {
    const centerX = control.x + control.w / 2;
    const centerY = control.y + 9;
    return centerX >= group.x && centerX <= group.x + group.w && centerY >= group.y && centerY <= group.y + group.h;
  };
  const owner = (control: LayoutControl) => named
    .filter(group => inside(control, group))
    .reduce<LayoutGroup | undefined>((best, group) => !best || group.w * group.h < best.w * best.h ? group : best, undefined);
  const assigned = new Set<string>();
  const groups: ProcessingGroup[] = named.map(group => {
    const controls = page.controls.filter(control => {
      const mine = owner(control) === group;
      if (mine) assigned.add(control.name);
      return mine;
    });
    return { title: titleCase(group.text), controls, order: group.x * 1000 + group.y };
  }).filter(group => group.controls.length);

  const unmatched = page.controls.filter(control => !assigned.has(control.name));
  if (unmatched.length) {
    const first = unmatched.reduce((best, control) => control.x < best.x ? control : best);
    groups.push({ title: page.title, controls: unmatched, order: first.x * 1000 + first.y });
  }
  return groups.sort((a, b) => a.order - b.order);
}
