import { test } from 'node:test';
import assert from 'node:assert/strict';
import { METER_BAR_WIDTH, METER_MONO_WELL_WIDTH, METER_PALETTE, METER_WELL_WIDTH, MeterMotion, groups, meterCanvasWidth, meterChannelLayout, meterGroupWidth, meterPercent, visibleGroups, conditionHolds, meterModelFromProfile } from '../src/meters.ts';
test('meters show each firmware record at once and expire when data stops',()=>{
 const m=new MeterMotion();assert.equal(m.sample(0),null);
 m.push(Array(112).fill(0),100);assert.equal(m.sample(100)![1],0);
 m.push(Array(112).fill(100),200);
 assert.equal(m.sample(201)![1],meterPercent(1,100),'the original client sets the bar to the record, no smoothing');
 assert.equal(m.sample(1401),null);
 m.push(Array(112).fill(20),1500);assert.equal(m.sample(1516)![1],meterPercent(1,20));
 m.clear();assert.equal(m.sample(1517),null);
});
test('peak markers are the firmware peak hold (bank 2) when the record has one',()=>{
 const m=new MeterMotion();
 const values=Array(112).fill(0).map((v,i)=>i===1?40:i===5?10:v);
 const peaks=Array(112).fill(0).map((v,i)=>i===1?60:i===5?30:v);
 m.push(values,0,peaks);m.sample(0);
 assert.equal(m.peakValues()[1],meterPercent(1,60));
 assert.equal(m.peakValues()[5],30);
 m.push(values,50,Array(112).fill(0));m.sample(50);
 assert.equal(m.peakValues()[1],meterPercent(1,40),'a peak never sits below its bar');
 m.push(values,100,[1,2]);m.sample(100);
 assert.equal(m.peakValues()[5],10,'a malformed peak record falls back to the lane hold');
});
test('gate lamps light for any value above 0, as the client two-step lamp',()=>{
 const gates=Object.fromEntries(groups.filter(g=>g.gate).map(g=>[g.name,g.gate!.id]));
 assert.deepEqual(gates,{'AGC':62,'FM Gain Reduction':63,'FM 2-Band Gain Reduction':63,'FM Loudness':111,'HD Gain Reduction':38,'HD 2-Band Gain Reduction':38});
 const m=new MeterMotion();m.push(Array(112).fill(0).map((v,i)=>i===62?127:v),0);
 assert.equal(m.gated(62),true);assert.equal(m.gated(63),false);
 m.clear();assert.equal(m.gated(62),false);
});
test('invalid or nonfinite frames clear meters',()=>{
 const m=new MeterMotion();m.push(Array(112).fill(24),10);assert.equal(m.sample(20)![1],meterPercent(1,24));
 m.push([1,2],30);assert.equal(m.sample(31),null);
 m.push(Array(112).fill(NaN),40);assert.equal(m.sample(41),null);
});
test('each meter lane holds and releases its own peak marker',()=>{
 const m=new MeterMotion();
 m.push(Array(112).fill(0).map((value,index)=>index===5?90:index===14?60:value),0);m.sample(0);
 assert.equal(m.peakValues()[5],90);
 assert.equal(m.peakValues()[14],60);
 m.push(Array(112).fill(0).map((value,index)=>index===5?20:index===14?80:value),500);m.sample(500);
 assert.equal(m.peakValues()[5],90,'peak must hold above the falling meter');
 assert.equal(m.peakValues()[14],80,'a separate lane must capture its own later peak');
 m.sample(1000);assert.equal(m.peakValues()[14],80,'the later lane peak must retain its own hold time');
 m.sample(1100);assert(m.peakValues()[5]<90&&m.peakValues()[5]>=20,'peak must release toward the current level');
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
 const fiveBand=groups.filter(group=>!group.name.includes('2-Band') && group.name!=='HF Limiting');
 assert.deepEqual(fiveBand.map(group=>group.referenceWidth),[71.021,95.541,73.042,169.542,66.542,67.041,53.042,59.541,169.542,72.042,65.042,65.542,72.52]);
 assert(Math.abs(meterCanvasWidth(fiveBand)-1100)<1e-9);
 const paired=meterChannelLayout(groups.find(group=>group.name==='FM Gain Reduction')!)[0];
 assert.equal(paired.lanes.length,2);
 assert(paired.lanes.every(lane=>lane.width===METER_BAR_WIDTH));
});

test('5700i band meters follow the structure in the on-air document, never both',()=>{
 const names=(values?:Record<string,string>)=>visibleGroups('Both',undefined,values).map(g=>g.name);
 const fiveBand={'Processing:B1 ATTACK':'20','Processing:HD B1 ATTACK':'20','Processing:AGC':'On'};
 const twoBand={'Processing:2B DRIVE':'0','Processing:HD 2B DRIVE':'0','Processing:AGC':'On'};
 assert(!names().some(n=>n.includes('Gain Reduction')),'no band meters before the document arrives');
 assert.deepEqual(names(fiveBand).filter(n=>n.includes('Gain Reduction')),['FM Gain Reduction','HD Gain Reduction']);
 assert.deepEqual(names(twoBand).filter(n=>n.includes('Gain Reduction')),['FM 2-Band Gain Reduction','HD 2-Band Gain Reduction']);
 assert.deepEqual(visibleGroups('Both',undefined,twoBand).find(g=>g.name==='FM 2-Band Gain Reduction')!.channels.map(c=>c.ids),[[64],[65]]);
 assert(Math.abs(meterCanvasWidth(visibleGroups('Both',undefined,fiveBand))-1100)<1e-9);
});
test('a presence condition needs the document of its own scope',()=>{
 const condition={field:'B1 ATTACK',present:false};
 assert.equal(conditionHolds(condition,{}),false);
 assert.equal(conditionHolds(condition,{'System:B1 ATTACK':'1'}),false);
 assert.equal(conditionHolds(condition,{'Processing:AGC':'On'}),true);
 assert.equal(conditionHolds({...condition,present:true},{'Processing:B1 ATTACK':'20'}),true);
});
test('every model profile carries its client gate lamps: AGC, multiband and HD',async()=>{
 const {readFileSync}=await import('node:fs');
 const load=(path:string)=>meterModelFromProfile(JSON.parse(readFileSync(new URL(`../../../profiles/${path}/meters.json`,import.meta.url),'utf8')));
 const gate=(path:string,name:string)=>load(path).groups.find(g=>g.name===name)?.gate?.id;
 // From each client's meter registration (push index; push "o_gateIndicate*").
 assert.equal(gate('6300/4.1.1.19','AGC'),69);
 assert.equal(gate('6300/4.1.1.19','2-Band Gain Reduction'),70);
 assert.equal(gate('6300/4.1.1.19','Gain Reduction'),70);
 assert.equal(gate('8700hd/1.0.2.161','2-Band HD Gain Reduction'),38);
 assert.equal(gate('8700i/1.5.1.11','Gain Reduction'),63);
 assert.equal(gate('5500i/3.1.0.2','2-Band Gain Reduction'),37);
 assert.equal(gate('2300/2.0.3.85','Gain Reduction'),39);
 assert.equal(load('6300/4.1.1.19').groups.find(g=>g.name==='AGC')?.gate?.vertical,true);
});
