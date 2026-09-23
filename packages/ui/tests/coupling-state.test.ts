import { test } from 'node:test';
import assert from 'node:assert/strict';
import { couplingConfirmationIsStale, type CouplingConfirmation } from '../src/coupling-state.ts';

const field={index:1,value:{type:'Choice' as const,value:'FM->HD'}};
const confirmation: CouplingConfirmation={field,session:'session-a',preset:'preset-a'};

test('coupling confirmation becomes stale when session, preset or device state changes',()=>{
 assert.equal(couplingConfirmationIsStale(confirmation,field,'session-a','preset-a'),false);
 assert.equal(couplingConfirmationIsStale(confirmation,field,'session-b','preset-a'),true);
 assert.equal(couplingConfirmationIsStale(confirmation,field,'session-a','preset-b'),true);
 assert.equal(couplingConfirmationIsStale(confirmation,{...field,index:0},'session-a','preset-a'),true);
});
