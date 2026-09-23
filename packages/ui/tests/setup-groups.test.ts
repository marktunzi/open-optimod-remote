import { test } from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { groupSetupFields, setupCurrentValue, setupFieldKind, setupLabel } from '../src/setup-groups.ts';

test('setup fields are placed in familiar functional groups without loss',()=>{
 const fields={
  'INPUT A OR D':1,'AO1 SOURCE':2,'PILOT LEVEL':3,'HD BW':4,
  'NETWORK IP ADDRESS':5,'TIME SERVER':6,'REMOTE CONTACT 1':7,
  'RDS PROGRAM NAME':8,'PRIM SNMP ADDRESS':9,'CONTRAST':10,
  'KANTAR ENABLE':11,'STATION ID':12,
 };
 const groups=groupSetupFields(fields);
 assert.deepEqual(groups.map(g=>g.name),[
  'Audio Input','Audio Outputs','FM Transmission','HD & Diversity',
  'Network & Time','Remote Control','RDS','SNMP','Display',
  'Identification & Ratings',
 ]);
 assert.deepEqual(groups.flatMap(g=>g.fields.map(([name])=>name)).sort(),Object.keys(fields).sort());
 assert.deepEqual(groups.flatMap(g=>g.sections.flatMap(section=>section.fields.map(([name])=>name))).sort(),Object.keys(fields).sort());
 assert.equal(groups.find(g=>g.name==='RDS')!.fields[0][0],'RDS PROGRAM NAME');
 assert.equal(groups.find(g=>g.name==='Audio Input')!.sections[0].name,'Source Selection');
 assert.equal(setupLabel('DI REF LEVEL'),'Digital Input Reference Level');
 assert.equal(setupLabel('RDS PROGRAM NAME'),'Program Service Name');
 assert.equal(setupLabel('KANTAR CHANNEL ID'),'Kantar Channel ID');
 assert.equal(setupLabel('RATINGS CBET CSID'),'Ratings CBET CSID');
 assert.equal(setupCurrentValue('undefined'),'Not Configured');
 assert.equal(setupCurrentValue(''),'Not Configured');
 assert.equal(setupCurrentValue('Active'),'Active');
 assert.equal(setupFieldKind('ACTUAL A OR D'),'status');
 assert.equal(setupFieldKind('AO1 SOURCE'),'routing');
});

test('the complete committed system inventory is retained and representative fields are grouped semantically',()=>{
 const reference=readFileSync(new URL('../../../docs/reference/observed-fields.md',import.meta.url),'utf8');
 const names=[...reference.matchAll(/^\| S-\d+ \| ([^|]+?) \|/gm)].map(match=>match[1]);
 assert.equal(names.length,178);
 const groups=groupSetupFields(Object.fromEntries(names.map((name,index)=>[name,index])));
 assert.equal(groups.flatMap(group=>group.fields).length,178);
 assert.equal(new Set(groups.flatMap(group=>group.fields.map(([name])=>name))).size,178);
 const category=(field:string)=>groups.find(group=>group.fields.some(([name])=>name===field))?.name;
 const section=(field:string)=>groups.flatMap(group=>group.sections).find(item=>item.fields.some(([name])=>name===field))?.name;
 assert.equal(category('AO PRE-OUT'),'Audio Outputs');
 assert.equal(category('FM BS1770 LDNES CTRL THR'),'FM Transmission');
 assert.equal(category('ITU412 THR'),'FM Transmission');
 assert.equal(category('BS1770 LDNES CTRL THR'),'HD & Diversity');
 assert.equal(section('FM BS1770 SAFETY LIMITER'),'Loudness Protection');
});
