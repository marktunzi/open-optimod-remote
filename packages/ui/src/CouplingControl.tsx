import { useState } from 'react';
import type { Action, Definition, Field } from './model';
import { couplingConfirmationIsStale, type CouplingConfirmation } from './coupling-state';

function CouplingLabel() {
  return <strong aria-hidden="true"><span>FM</span><svg viewBox="603 78 8 7.2" focusable="false">
    <path d="M609.499 78.6875C609.08 78.2693 608.402 78.2693 607.984 78.6875L606.714 79.9568L606.131 79.3735L607.4 78.1041C608.141 77.3637 609.342 77.3637 610.082 78.1041C610.822 78.8446 610.822 80.0453 610.082 80.7858L608.813 82.0551L608.229 81.4717L609.499 80.2024C609.917 79.7842 609.917 79.1058 609.499 78.6875Z"/>
    <path d="M605.764 80.9432L604.495 82.2125C604.076 82.6307 604.076 83.3091 604.495 83.7274C604.913 84.1456 605.591 84.1456 606.01 83.7274L607.279 82.4581L607.862 83.0414L606.593 84.3107C605.852 85.0512 604.652 85.0512 603.911 84.3107C603.171 83.5703 603.171 82.3696 603.911 81.6291L605.181 80.3598L605.764 80.9432Z"/>
    <path d="M605.574 83.2111L609.009 79.7769L608.425 79.1935L604.991 82.6277L605.574 83.2111Z"/>
  </svg><span>HD</span></strong>;
}

export function CouplingControl({ field, definition, connected, writeEnabled, busy, sessionId, presetName, action }: {
  field?: Field; definition?: Definition; connected: boolean; writeEnabled: boolean;
  busy: boolean; sessionId?: string | null; presetName: string; action: Action;
}) {
  const [confirmation, setConfirmation] = useState<CouplingConfirmation | null>(null);
  if (!field || !definition) return <section className="coupling-control" aria-label="FM to HD processing coupling">
    <button className="coupling-trigger" disabled aria-label="FM to HD coupling unavailable" title="FM to HD coupling is unavailable">
      <span className="status-pill" aria-hidden="true"><i/></span><CouplingLabel/>
    </button>
  </section>;
  const coupled = field.value.value === 'FM->HD';
  const target = coupled
    ? definition.values.findIndex(value => value.value === 'Indepen.')
    : definition.values.findIndex(value => value.value === 'FM->HD');
  const stale = couplingConfirmationIsStale(confirmation,field,sessionId,presetName);
  const disabled = !connected || !writeEnabled || busy || !sessionId || target < 0;
  const apply = async () => {
    if (!confirmation || disabled || stale) return;
    const ok = await action('change', {
      scope: 'Processing', name: 'HD COUPLING', index: target,
      expected: confirmation.field, expected_session: confirmation.session, expected_preset: confirmation.preset,
    });
    if (ok) setConfirmation(null);
  };
  return <section className={`coupling-control${confirmation?' confirming':''}`} aria-label="FM to HD processing coupling">
    <button className={`coupling-trigger${coupled?' is-coupled':''}`} disabled={disabled} aria-pressed={coupled}
      aria-label={coupled?'FM to HD linked; activate to decouple':'FM and HD independent; activate to couple'}
      title={coupled?'FM and HD are linked. Click to decouple.':'FM and HD are independent. Click to couple.'}
      onClick={() => setConfirmation({field,session:sessionId!,preset:presetName})}>
      <span className="status-pill" aria-hidden="true"><i/></span><CouplingLabel/>
    </button>
    {confirmation && <div className="coupling-confirmation" role="alert">
      <span>{coupled ? 'HD will stop following FM and become editable independently.' : 'HD will follow FM. Its current processing may change immediately.'}</span>
      {stale && <span>The device state changed. Cancel and review the current setting.</span>}
      <button disabled={disabled || stale} onClick={() => void apply()}>{coupled ? 'Confirm independence' : 'Confirm FM → HD coupling'}</button>
      <button disabled={busy} onClick={() => setConfirmation(null)}>Cancel</button>
    </div>}
  </section>;
}
