import type { ReactNode } from 'react';

/** Line icons of the Figma Meters window action bar (24 pt grid). */
const ICONS: Record<string, ReactNode> = {
  setup: <><circle cx="12" cy="12" r="4.2"/><path d="M12 3.5v2M12 18.5v2M3.5 12h2M18.5 12h2M6 6l1.4 1.4M16.6 16.6 18 18M6 18l1.4-1.4M16.6 7.4 18 6"/><path d="m10.4 13.6 3.2-3.2"/></>,
  presets: <><path d="M7 4.5h10M5.5 7h13"/><rect x="4" y="9.5" width="16" height="10" rx="2"/><circle cx="16.5" cy="17" r="3.2" fill="currentColor" stroke="none"/><path d="M16.5 15.4v3.2M14.9 17h3.2" stroke="#4d4d4d"/></>,
  link: <><path d="M10.2 13.8a3.2 3.2 0 0 0 4.5 0l3-3a3.2 3.2 0 0 0-4.5-4.5l-1 1"/><path d="M13.8 10.2a3.2 3.2 0 0 0-4.5 0l-3 3a3.2 3.2 0 0 0 4.5 4.5l1-1"/></>,
  recall: <><path d="M19 12a7 7 0 1 1-2.6-5.45"/><path d="M16.8 3.2v3.6h-3.6"/></>,
  settings: <><path d="M4 7.5h16M4 16.5h16"/><circle cx="8.5" cy="7.5" r="2.3" fill="#313131"/><circle cx="15.5" cy="16.5" r="2.3" fill="#313131"/></>,
};

export type FigmaIconName = keyof typeof ICONS;

export function FigmaIcon({ name }: { name: FigmaIconName }) {
  return <svg viewBox="0 0 24 24" aria-hidden="true" focusable="false" fill="none" stroke="currentColor"
    strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round">{ICONS[name]}</svg>;
}
