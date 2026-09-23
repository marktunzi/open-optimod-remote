import { test } from 'node:test';
import assert from 'node:assert/strict';
import { METER_BAR_WIDTH, METER_MONO_WELL_WIDTH, METER_PALETTE, METER_WELL_WIDTH, MeterMotion, groups, meterCanvasWidth, meterChannelLayout, meterGroupWidth, meterPercent, visibleGroups } from '../src/meters.ts';
test('meter movement is monotonic, bounded and expires when data stops',()=>{
 const m=new MeterMotion();assert.equal(m.sample(0),null);
 m.push(Array(112).fill(0),100);m.sample(100);
 m.push(Array(112).fill(100),200);const a=m.sample(216)![1];const b=m.sample(232)![1];
 assert(a>0&&a<100);assert(b>a&&b<100);
 assert.equal(m.sample(1401),null);
 m.push(Array(112).fill(20),1500);assert(m.sample(1516)![1]<=meterPercent(1,20),'old device values must not survive a stale interval');
 m.clear();assert.equal(m.sample(1517),null);
});
test('meter ballistics bridge device packets without a visible first-frame jump',()=>{
 const m=new MeterMotion();
 m.push(Array(112).fill(0),0);m.sample(0);
 m.sample(16);m.sample(32);m.sample(48);
 m.push(Array(112).fill(100),50);
 const a=m.sample(66)![1],b=m.sample(82)![1],c=m.sample(98)![1];
 assert(a>0&&a<45,`first rendered step was ${a}`);
 assert(b>a&&c>b&&c>55);
 assert(Math.max(a,b-a,c-b)<45,'motion must be distributed across animation frames');
});
test('invalid or nonfinite frames clear meters, reduced motion uses actual values',()=>{
 const m=new MeterMotion();m.push(Array(112).fill(24),10);assert.equal(m.sample(20,true)![1],meterPercent(1,24));
 m.push([1,2],30);assert.equal(m.sample(31),null);
 m.push(Array(112).fill(NaN),40);assert.equal(m.sample(41),null);
});
test('each meter lane holds and releases its own peak marker',()=>{
 const m=new MeterMotion();
 m.push(Array(112).fill(0).map((value,index)=>index===5?90:index===14?60:value),0);m.sample(0,true);
 assert.equal(m.peakValues()[5],90);
 assert.equal(m.peakValues()[14],60);
 m.push(Array(112).fill(0).map((value,index)=>index===5?20:index===14?80:value),500);m.sample(500,true);
 assert.equal(m.peakValues()[5],90,'peak must hold above the falling meter');
 assert.equal(m.peakValues()[14],80,'a separate lane must capture its own later peak');
 m.sample(1000,true);assert.equal(m.peakValues()[14],80,'the later lane peak must retain its own hold time');
 m.sample(1100,true);assert(m.peakValues()[5]<90&&m.peakValues()[5]>=20,'peak must release toward the current level');
 assert.equal(m.peakValues()[14],80,'the later peak must still be held independently');
 m.clear();assert.deepEqual(m.peakValues(),[]);
});
test('FM and HD output and stereo reduction channels remain distinct',()=>{
 assert.deepEqual(groups.find(g=>g.name==='FM Output')!.channels.map(c=>c.ids),[[27,28]]);
 assert.deepEqual(groups.find(g=>g.name==='HD Output')!.channels.map(c=>c.ids),[[51,52]]);
 assert.deepEqual(groups.find(g=>g.name==='FM Gain Reduction')!.channels[0].ids,[5,14]);
 assert.deepEqual(groups.find(g=>g.name==='HD Gain Reduction')!.channels[0].ids,[39,44]);
});
test('reference curves keep HD output headroom and reverse loudness reduction', async()=>{
 const {meterPercent}=await import('../src/meters.ts');
 assert.equal(meterPercent(51,157),83);
 assert.equal(meterPercent(1,50),79);
 assert.equal(meterPercent(57,100),0);
 assert.equal(meterPercent(19,20),0);
});

test('FM and HD display filters keep shared meters without rerouting',()=>{
 assert(visibleGroups('FM').every(g=>!g.name.startsWith('HD ')));
 assert(visibleGroups('HD').some(g=>g.name==='Input'));
 assert(!visibleGroups('HD').some(g=>g.name==='Composite'));
 assert.deepEqual(visibleGroups('HD').find(g=>g.name==='Loudness GR')!.channels.flatMap(c=>c.ids),[57]);
 assert.deepEqual(visibleGroups('FM').find(g=>g.name==='Loudness GR')!.channels.flatMap(c=>c.ids),[60]);
});

test('meter lanes and stereo wells keep the fixed dimensions from the final artwork',()=>{
 const layouts=groups.flatMap(group=>meterChannelLayout(group));
 assert(layouts.length>0);
 assert(layouts.flatMap(channel=>channel.lanes).every(lane=>lane.width===METER_BAR_WIDTH));
 assert(layouts.filter(channel=>channel.lanes.length===2).every(channel=>channel.wellWidth===METER_WELL_WIDTH));
 assert(layouts.filter(channel=>channel.lanes.length===1).every(channel=>channel.wellWidth===METER_MONO_WELL_WIDTH));
 for(const view of ['FM','HD','Both'] as const){
  const visible=visibleGroups(view);
  assert.equal(meterCanvasWidth(visible),visible.reduce((sum,group)=>sum+meterGroupWidth(group),0));
 }
});

test('approved SVG fixes meter geometry and palette exactly',()=>{
 assert.equal(METER_WELL_WIDTH,23.5);
 assert.equal(METER_MONO_WELL_WIDTH,13);
 assert.equal(METER_BAR_WIDTH,8);
 assert.deepEqual(METER_PALETTE,{
  backgroundTop:'#353432',backgroundBottom:'#32312F',panelTop:'#20201E',panelBottom:'#252523',well:'#141414',separator:'#171716',separatorHighlight:'rgba(255,255,255,.055)',
  ticks:'#656565',label:'#9F9F9F',blue:'#0088FF',amber:'#FFCC00',
  red:'#FF383C',green:'#76D672',live:'#34C759',magenta:'#A62E9C',lcdTop:'#101A24',lcdBottom:'#182431',lcdText:'#7E95B1',
 });
 assert.deepEqual(groups.map(group=>group.referenceWidth),[71.021,95.541,73.042,169.542,66.542,67.041,53.042,59.541,169.542,72.042,65.042,65.542,72.52]);
 assert(Math.abs(meterCanvasWidth(groups)-1100)<1e-9);
 const paired=meterChannelLayout(groups.find(group=>group.name==='FM Gain Reduction')!)[0];
 assert.equal(paired.lanes.length,2);
 assert(paired.lanes.every(lane=>lane.width===METER_BAR_WIDTH));
});
