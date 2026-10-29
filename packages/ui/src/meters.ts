import { curves } from './meter-curves.ts';
export type MeterChannel = {label: string; ids: number[]; laneLabels?: string[]};
export type MeterGroup = {
 name: string;
 kind: 'level' | 'reduction' | 'loudness' | 'enhance' | 'composite';
 /** Axis labels as [position from the top 0-1, label], where a profile gives the model's own scale. */
 ticks?: [number, string][];
 /** Shown only while the processor is in this state (e.g. the 8400's 2B/5B SWITCH). */
 when?: MeterCondition;
 /** A gate lamp of the original client: lit while this channel is not 0. */
 gate?: MeterGate;
 referenceWidth: number;
 contentOffset: number;
 channels: MeterChannel[];
};
/**
 * The 5700i client draws a gate lamp as a two-step bargraph (table {100, 0}):
 * lit for any value above 0. The firmware sends 0 or 127.
 */
export type MeterGate = {id: number; label: string; vertical?: boolean};
export const METER_WELL_WIDTH = 23.5;
export const METER_MONO_WELL_WIDTH = 13;
export const METER_BAR_WIDTH = 8;
const METER_LANE_GAP = 2.5;
export const METER_PALETTE = {
 backgroundTop:'#353432',backgroundBottom:'#32312F',panelTop:'#20201E',panelBottom:'#252523',well:'#141414',separator:'#171716',separatorHighlight:'rgba(255,255,255,.055)',
 ticks:'#656565',label:'#9F9F9F',blue:'#0088FF',amber:'#FFCC00',
 red:'#FF383C',green:'#76D672',live:'#34C759',magenta:'#A62E9C',lcdTop:'#101A24',lcdBottom:'#182431',lcdText:'#7E95B1',
} as const;
export type MeterChannelLayout = {
 label: string;
 center: number;
 wellLeft: number;
 wellWidth: number;
 lanes: {id: number; label?: string; left: number; width: number}[];
};
const mono = (label: string, id: number) => ({label,ids:[id]});
const pair = (label: string, left: number, right: number, laneLabels?: string[]) => ({label,ids:[left,right],laneLabels});
/**
 * The processing structure on air, read from the processor's own on-air
 * document: a 5-band document carries the band fields (`B1 ATTACK`), a 2-band
 * document the 2-band fields (`2B DRIVE`), never both (every factory preset of
 * the 5500i, 6300, 5700i, 8700HD and 8700i). HD has its own fields.
 */
export const FM_FIVE_BAND: MeterCondition = {field: 'B1 ATTACK', present: true};
export const FM_TWO_BAND: MeterCondition = {field: '2B DRIVE', present: true};
export const HD_FIVE_BAND: MeterCondition = {field: 'HD B1 ATTACK', present: true};
export const HD_TWO_BAND: MeterCondition = {field: 'HD 2B DRIVE', present: true};
export const groups: MeterGroup[] = [
 {name:'Input',kind:'level',referenceWidth:71.021,contentOffset:31,channels:[pair('',1,2,['L','R'])]},
 // Gate lamps from the original client's meter dialogs (5-band 247, 2-band 373):
 // AGC Gate 62, FM Multi Gate 63, HD Multi Gate 38, MPX Pwr Gate 111.
 {name:'AGC',kind:'reduction',referenceWidth:95.541,contentOffset:24.521,channels:[pair('B',3,10),pair('M',4,11)],gate:{id:62,label:'GATED',vertical:true}},
 {name:'FM HF Enhance',kind:'enhance',referenceWidth:73.042,contentOffset:31.521,channels:[pair('',34,35,['L','R'])]},
 {name:'FM Gain Reduction',kind:'reduction',referenceWidth:169.542,contentOffset:25.021,channels:[pair('1',5,14),pair('2',6,15),pair('3',7,16),pair('4',8,17),pair('5',9,18)],when:FM_FIVE_BAND,gate:{id:63,label:'GATED'}},
 {name:'FM 2-Band Gain Reduction',kind:'reduction',referenceWidth:67,contentOffset:25,channels:[mono('B',64),mono('M',65)],when:FM_TWO_BAND,gate:{id:63,label:'GATED'}},
 // The original 5700i client's 2-band view (dialog 373) adds the HF limiter, values 21/22 (28 October 2026, live).
 {name:'HF Limiting',kind:'reduction',referenceWidth:72.042,contentOffset:27.521,channels:[pair('',21,22,['L','R'])],when:FM_TWO_BAND},
 {name:'FM Loudness',kind:'loudness',referenceWidth:66.542,contentOffset:24.521,channels:[pair('',59,58)],gate:{id:111,label:'MPX GATED'}},
 {name:'FM Output',kind:'level',referenceWidth:67.041,contentOffset:24.52,channels:[pair('',27,28,['L','R'])]},
 {name:'Composite',kind:'composite',referenceWidth:53.042,contentOffset:24.521,channels:[mono('',29)]},
 {name:'HD HF Enhance',kind:'enhance',referenceWidth:59.541,contentOffset:24.521,channels:[pair('',36,37,['L','R'])]},
 {name:'HD Gain Reduction',kind:'reduction',referenceWidth:169.542,contentOffset:25.521,channels:[pair('1',39,44),pair('2',40,45),pair('3',41,46),pair('4',42,47),pair('5',43,48)],when:HD_FIVE_BAND,gate:{id:38,label:'GATED'}},
 {name:'HD 2-Band Gain Reduction',kind:'reduction',referenceWidth:67,contentOffset:25,channels:[mono('B',49),mono('M',50)],when:HD_TWO_BAND,gate:{id:38,label:'GATED'}},
 {name:'HD Limiting',kind:'reduction',referenceWidth:72.042,contentOffset:27.521,channels:[pair('',19,20,['L','R'])]},
 {name:'Loudness GR',kind:'reduction',referenceWidth:65.042,contentOffset:27.021,channels:[mono('',57)]},
 {name:'HD Loudness',kind:'loudness',referenceWidth:65.542,contentOffset:24.522,channels:[pair('',56,55)]},
 {name:'HD Output',kind:'level',referenceWidth:72.52,contentOffset:26.52,channels:[pair('',51,52,['L','R'])]},
];

export type MeterView = 'FM' | 'HD' | 'Both';
/** Meter groups, raw-value conversion and accepted record length for one processor profile. */
export type MeterModel = {
 groups: MeterGroup[];
 percent: (id: number, raw: number) => number;
 minValues: number;
 maxValues: number;
 reference: boolean;
};
/** Meters as delivered in a model profile (see scripts/extract_pc_remote.py). */
export type ProfileMeters = {
 max_values: number;
 groups: {name: string; kind: MeterGroup['kind']; channels: MeterChannel[]; ticks?: [number, string][]; when?: MeterCondition; gate?: MeterGate}[];
 curves?: Record<string, number[]>;
};
/**
 * A setting a meter group depends on: `value` must match, `not` must not, or
 * with `present` the field must be (or not be) in the processor's document.
 */
export type MeterCondition = {scope?: 'Processing' | 'System'; field: string; value?: string; not?: string; present?: boolean};
/** Current setting values by `Scope:NAME`, for meter conditions. */
export type SettingValues = Record<string, string>;
export function settingValues(processing?: {fields: Record<string, {value: {value: unknown}}>} | null, system?: {fields: Record<string, {value: {value: unknown}}>} | null): SettingValues {
 const values: SettingValues = {};
 for (const [scope, document] of [['Processing', processing], ['System', system]] as const)
  for (const [name, field] of Object.entries(document?.fields || {})) values[`${scope}:${name}`] = String(field.value.value);
 return values;
}
/** A group with a condition is shown only once the setting is known and matches. */
export function conditionHolds(condition: MeterCondition | undefined, values: SettingValues | undefined): boolean {
 if (!condition) return true;
 const scope = condition.scope || 'Processing';
 const value = values?.[`${scope}:${condition.field}`];
 if (condition.present !== undefined) {
  // Absence only counts once the processor has sent its document.
  const known = Object.keys(values || {}).some(key => key.startsWith(`${scope}:`));
  return known && (value !== undefined) === condition.present;
 }
 if (value === undefined) return false;
 return condition.value !== undefined ? value === condition.value : value !== condition.not;
}
export function visibleGroups(view: MeterView, model: MeterModel = REFERENCE_METERS, values?: SettingValues): MeterGroup[] {
 return model.groups.filter(g => conditionHolds(g.when, values)).filter(g => view === 'Both' || (!g.name.startsWith(view === 'FM' ? 'HD ' : 'FM ') && !(view === 'HD' && g.kind === 'composite')))
  .map(g => model.reference && g.name === 'Loudness GR' && view === 'FM'
   ? {...g,channels:[mono('',60)]}
   : g);
}
const PROFILE_CONTENT_OFFSET = 25;
const PROFILE_GROUP_PADDING = 12;
function profileGroupWidth(group: Omit<MeterGroup,'referenceWidth'|'contentOffset'>): number {
 const wells = group.channels.map(c => c.ids.length === 1 ? METER_MONO_WELL_WIDTH : METER_WELL_WIDTH);
 const gap = group.name === 'AGC' ? 13.5 : 4;
 return PROFILE_CONTENT_OFFSET + wells.reduce((a, b) => a + b, 0) + gap * Math.max(0, wells.length - 1) + PROFILE_GROUP_PADDING;
}
function interpolate(curve: readonly number[], raw: number): number {
 const x=Math.max(0,Math.min(curve.length-1,raw)),low=Math.floor(x),high=Math.ceil(x);
 return curve[low]+(curve[high]-curve[low])*(x-low);
}
/** Builds a meter model from a profile; channels without a curve are linear 0–100. */
export function meterModelFromProfile(meters: ProfileMeters): MeterModel {
 const groups = meters.groups.map(group => {
  const channels = group.channels.map(c => ({label: c.label, ids: [...c.ids], laneLabels: c.laneLabels}));
  const base = {name: group.name, kind: group.kind, channels, ...(group.ticks ? {ticks: group.ticks.map(([position, label]) => [position, label] as [number, string])} : {}), ...(group.when ? {when: {...group.when}} : {})};
  // The client's gate lamp for this group (scripts/extract_pc_remote.py: client_gate_lamps).
  return {...base, ...(group.gate ? {gate: {...group.gate}} : {}), contentOffset: PROFILE_CONTENT_OFFSET, referenceWidth: profileGroupWidth(base)};
 });
 const curves = meters.curves || {};
 return {
  groups,
  percent: (id, raw) => {
   if (!Number.isFinite(raw)) return 0;
   const curve = curves[String(id)];
   return curve ? interpolate(curve, raw) : Math.max(0, Math.min(100, raw));
  },
  minValues: 1,
  // The service accepts up to 512 values; channels beyond a record are shown empty.
  maxValues: 512,
  reference: false,
 };
}
export function meterChannelLayout(group: MeterGroup): MeterChannelLayout[] {
 let left=0;
 const channelGap=group.name==='AGC'?13.5:4;
 return group.channels.map(channel=>{
  const wellWidth=channel.ids.length===1?METER_MONO_WELL_WIDTH:METER_WELL_WIDTH;
  const barsWidth=channel.ids.length*METER_BAR_WIDTH+Math.max(0,channel.ids.length-1)*METER_LANE_GAP;
  const inset=(wellWidth-barsWidth)/2;
  const lanes=channel.ids.map((id,index)=>({id,label:channel.laneLabels?.[index],left:left+inset+index*(METER_BAR_WIDTH+METER_LANE_GAP),width:METER_BAR_WIDTH}));
  const layout={label:channel.label,center:left+wellWidth/2,wellLeft:left,wellWidth,lanes};
  left+=wellWidth+channelGap;
  return layout;
 });
}
export function meterGroupWidth(group: MeterGroup): number {
 return group.referenceWidth;
}
export function meterCanvasWidth(meterGroups: MeterGroup[]): number {
 return meterGroups.reduce((width,group)=>width+meterGroupWidth(group),0);
}
export function meterPercent(id: number, raw: number): number {
 const key = [1,2,27,28].includes(id) ? 'level' : [51,52].includes(id) ? 'hdOutput'
  : [19,20].includes(id) ? 'limiting' : id===57 ? 'hdLoudnessGr' : id===60 ? 'fmLoudnessGr'
  : [55,56,58,59].includes(id) ? 'loudness' : null;
 if(!Number.isFinite(raw))return 0;
 if(!key)return Math.max(0,Math.min(100,raw));
 return interpolate(curves[key],raw);
}
/** The hardware-verified 5700i meters. */
export const REFERENCE_METERS: MeterModel = {groups, percent: meterPercent, minValues: 112, maxValues: 112, reference: true};
/**
 * Display percentages as the original PC Remote shows them: every record sets
 * each bar to the firmware's current value (bank 1) and its peak marker to the
 * firmware's peak hold (bank 2), with no smoothing in between (5700iPC.exe
 * bargraph SetPos 0x442d70, fed by 0x444440). Without firmware peaks a lane
 * holds its own peak for 900 ms and releases it at 35 points per second.
 */
export class MeterMotion {
 private model: MeterModel;
 constructor(model: MeterModel = REFERENCE_METERS) {this.model=model;}
 setModel(model: MeterModel) {this.model=model;this.clear();}
 private values: number[] = [];
 private firmwarePeaks: number[] | null = null;
 private shown: number[] = [];
 private peaks: number[] = [];
 private peakHeldAt: number[] = [];
 private received = -Infinity;
 private drawn = 0;
 clear() {this.values=[];this.firmwarePeaks=null;this.shown=[];this.peaks=[];this.peakHeldAt=[];this.received=-Infinity;this.drawn=0;}
 private valid(record: unknown): record is number[] {
  const {minValues,maxValues}=this.model;
  return Array.isArray(record)&&record.length>=minValues&&record.length<=maxValues&&record.every(v=>Number.isInteger(v)&&v>=0&&v<=255);
 }
 push(values: unknown, at: number, peaks?: unknown) {
  if (!this.valid(values)) {this.clear();return;}
  this.values=[...values];this.received=at;
  this.firmwarePeaks=this.valid(peaks)&&peaks.length===values.length?[...peaks]:null;
 }
 sample(at: number): number[] | null {
  if(!this.values.length||at-this.received>1200){this.clear();return null;}
  const dt=Math.max(0,Math.min(50,at-this.drawn))/1000;this.drawn=at;
  this.shown=this.values.map((raw,i)=>Math.max(0,Math.min(100,this.model.percent(i,raw))));
  const firmware=this.firmwarePeaks;
  this.peaks=this.shown.map((current,i)=>{
   if(firmware)return Math.max(current,Math.min(100,this.model.percent(i,firmware[i])));
   const peak=this.peaks[i]??current;
   if(current>=peak){this.peakHeldAt[i]=at;return current;}
   if(at-(this.peakHeldAt[i]??at)<=900)return peak;
   return Math.max(current,peak-35*dt);
  });
  return this.shown;
 }
 raw() {return this.values;}
 peakValues() {return this.peaks;}
 /** Whether a gate lamp is lit; false without a current record. */
 gated(id: number): boolean {return (this.values[id]??0)>0;}
}
