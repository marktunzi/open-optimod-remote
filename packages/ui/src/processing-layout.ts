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

function titleCase(value: string): string {
  return value
    .trim()
    .toLocaleLowerCase()
    .replace(/\b\w/g, character => character.toLocaleUpperCase());
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

  const assigned = new Set<string>();
  const groups: ProcessingGroup[] = named.map(group => {
    const controls = page.controls.filter(control => {
      const centerX = control.x + control.w / 2;
      const centerY = control.y + 9;
      const inside = centerX >= group.x && centerX <= group.x + group.w
        && centerY >= group.y && centerY <= group.y + group.h;
      if (inside) assigned.add(control.name);
      return inside;
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
