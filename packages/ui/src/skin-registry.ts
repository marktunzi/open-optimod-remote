export type ProcessorSkinId =
  | 'optimod-5700i'
  | 'optimod-5500i'
  | 'optimod-5500'
  | 'optimod-5700-fm'
  | 'optimod-5700-hd'
  | 'optimod-8500'
  | 'optimod-6300'
  | 'optimod-8600'
  | 'optimod-8700i'
  | 'optimod-9300'
  | 'optimod-9400';

export type ProcessorSkin = {
  id: ProcessorSkinId | 'neutral';
  productName: string;
  logo: string;
  material: string;
  pathLabel: string;
  meterGroups: string[];
};

export const PROCESSOR_SKINS: Record<ProcessorSkinId, ProcessorSkin> = {
  'optimod-5700i': {
    id: 'optimod-5700i',
    productName: 'OPTIMOD 5700i',
    logo: '/assets/optimod-5700i.svg',
    material: 'graphite-magenta',
    pathLabel: 'FM ↔ HD',
    meterGroups: ['Input', 'AGC', 'FM Gain Reduction', 'FM Output', 'HD Gain Reduction', 'HD Output'],
  },
  'optimod-5500i': {
    id: 'optimod-5500i',
    productName: 'OPTIMOD 5500i',
    logo: '/assets/optimod-5500i.svg',
    material: 'anthracite-teal',
    pathLabel: 'FM + Digital',
    meterGroups: ['Input', 'AGC', 'Multiband', 'Composite', 'Output'],
  },
  'optimod-5500': {
    id: 'optimod-5500',
    productName: 'OPTIMOD 5500',
    logo: '/assets/optimod-5500.svg',
    material: 'charcoal-amber',
    pathLabel: 'FM',
    meterGroups: ['Input', 'AGC', 'Multiband', 'Composite', 'FM Output'],
  },
  'optimod-5700-fm': {
    id: 'optimod-5700-fm',
    productName: 'OPTIMOD 5700 FM',
    logo: '/assets/optimod-5700-fm.svg',
    material: 'broadcast-violet',
    pathLabel: 'FM',
    meterGroups: ['Input', 'AGC', 'FM Gain Reduction', 'Loudness', 'FM Output'],
  },
  'optimod-5700-hd': {
    id: 'optimod-5700-hd',
    productName: 'OPTIMOD 5700 HD',
    logo: '/assets/optimod-5700-hd.svg',
    material: 'digital-cobalt',
    pathLabel: 'HD',
    meterGroups: ['Input', 'AGC', 'HD Gain Reduction', 'HD Limiting', 'HD Output'],
  },
  'optimod-8500': {
    id: 'optimod-8500',
    productName: 'OPTIMOD 8500',
    logo: '/assets/optimod-8500.svg',
    material: 'titanium-cyan',
    pathLabel: 'FM + HD',
    meterGroups: ['Input', 'AGC', 'FM Gain Reduction', 'FM Output', 'HD Gain Reduction', 'HD Output'],
  },
  'optimod-6300': {
    id: 'optimod-6300',
    productName: 'OPTIMOD 6300',
    logo: '/assets/optimod-6300.svg',
    material: 'digital-slate-green',
    pathLabel: 'Digital / TV',
    meterGroups: ['Input', 'AGC', 'Multiband', 'Loudness', 'Digital Output'],
  },
  'optimod-8600': {
    id: 'optimod-8600',
    productName: 'OPTIMOD 8600',
    logo: '/assets/optimod-8600.svg',
    material: 'titanium-electric-blue',
    pathLabel: 'FM + HD',
    meterGroups: ['Input', 'AGC', 'FM Gain Reduction', 'FM Output', 'HD Gain Reduction', 'HD Output'],
  },
  'optimod-8700i': {
    id: 'optimod-8700i',
    productName: 'OPTIMOD 8700i',
    logo: '/assets/optimod-8700i.svg',
    material: 'premium-graphite-red',
    pathLabel: 'FM + Digital',
    meterGroups: ['Input', 'AGC', 'FM Gain Reduction', 'MX Limiting', 'Digital Gain Reduction', 'Output'],
  },
  'optimod-9300': {
    id: 'optimod-9300',
    productName: 'OPTIMOD 9300',
    logo: '/assets/optimod-9300.svg',
    material: 'am-bronze',
    pathLabel: 'AM Mono',
    meterGroups: ['Input', 'AGC', 'Multiband', 'Positive Peak', 'Negative Peak', 'AM Output'],
  },
  'optimod-9400': {
    id: 'optimod-9400',
    productName: 'OPTIMOD 9400',
    logo: '/assets/optimod-9400.svg',
    material: 'am-burgundy',
    pathLabel: 'AM + Digital',
    meterGroups: ['Input', 'AGC', 'AM Gain Reduction', 'AM Output', 'Digital Gain Reduction', 'Digital Output'],
  },
};

const NEUTRAL_SKIN: ProcessorSkin = {
  id: 'neutral',
  productName: 'OPTIMOD',
  logo: '/assets/optimod-neutral.svg',
  material: 'neutral-graphite',
  pathLabel: 'Processor',
  meterGroups: ['Input', 'Processing', 'Output'],
};

export function resolveProcessorSkin(id?: string | null): ProcessorSkin {
  return (id && PROCESSOR_SKINS[id as ProcessorSkinId]) || NEUTRAL_SKIN;
}
