import { useMemo, useState } from 'react';
import { SettingEditor } from './SettingEditor';
import { display, type Action, type Definition, type Field } from './model';
import { groupSetupFields, setupCurrentValue, setupFieldKind, setupLabel } from './setup-groups';

export function Setup({ records, definitions, connected, writeEnabled, busy, identity, sessionId, action, openOutputs }: {
  records?: Record<string, Field>; definitions: Definition[]; connected: boolean;
  writeEnabled: boolean; busy: boolean; identity: string; sessionId?: string | null; action: Action;
  openOutputs: () => void;
}) {
  const groups = useMemo(() => groupSetupFields(records || {}), [records]);
  const definitionMap = useMemo(() => new Map(definitions.filter(item=>item.scope==='System').map(item=>[item.name,item])), [definitions]);
  const [selected, setSelected] = useState('Audio Input');
  const [filter, setFilter] = useState('');
  const query = filter.trim().toLocaleLowerCase();
  const activeName = groups.some(group=>group.name===selected) ? selected : groups[0]?.name;
  const shown = query ? groups.map(group=>{
    const sections=group.sections.map(section=>({...section,fields:section.fields.filter(([name])=>
      `${setupLabel(name)} ${name} ${section.name} ${group.name}`.toLocaleLowerCase().includes(query),
    )})).filter(section=>section.fields.length);
    return {...group,sections,fields:sections.flatMap(section=>section.fields)};
  }).filter(group=>group.fields.length) : groups.filter(group=>group.name===activeName);
  const resultCount=shown.reduce((count,group)=>count+group.fields.length,0);

  return <section className="settings setup-panel">
    <div className="pageheading setup-pageheading">
      <div><span className="eyebrow">Processor Configuration</span><h2>System Setup</h2>
        <p>Choose a category to view related settings. Physical source assignment is managed in Outputs.</p></div>
      <label className="setup-search"><span>Search All Settings</span>
        <input name="setup-search" autoComplete="off" placeholder="Search by name or function…" value={filter} onChange={event=>setFilter(event.target.value)} />
      </label>
    </div>
    <div className="setup-layout">
      <nav className="setup-nav" aria-label="System setup categories">
        {groups.map(group=><button key={group.name} aria-pressed={!query&&activeName===group.name} onClick={()=>{setSelected(group.name);setFilter('');}}>
          <span>{group.name}</span><small>{group.fields.length}</small>
        </button>)}
      </nav>
      <div className="setup-content">
        {query && <div className="setup-results"><strong role="status" aria-live="polite">{resultCount} {resultCount===1?'setting':'settings'} found</strong><button onClick={()=>setFilter('')}>Clear Search</button></div>}
        {!shown.length && <div className="setup-empty" role="status" aria-live="polite"><strong>No Matching Settings</strong><p>Try a device term such as “input”, “RDS”, “network” or “meter”.</p></div>}
        {shown.map(group=><article className="setup-group" key={group.name} aria-labelledby={`setup-${group.name.replaceAll(' ','-')}`}>
          <header className="setup-group-heading"><div><h2 id={`setup-${group.name.replaceAll(' ','-')}`}>{group.name}</h2><p>{group.description}</p></div><span>{group.fields.length} settings</span></header>
          {group.sections.map(section=><section className="setup-section" key={section.name}>
            <div className="setup-section-heading"><h3>{section.name}</h3><span>{section.fields.length}</span></div>
            <div className="setup-setting-list">
              {section.fields.map(([name,field])=>{
                const definition=definitionMap.get(name);
                const kind=setupFieldKind(name);
                return <div className="setup-setting" key={name} title={name}>
                  <div className="setup-setting-name"><strong>{setupLabel(name)}</strong>
                    {kind==='status' && <small>Device status</small>}
                    {kind==='routing' && <small>Physical source assignment</small>}
                  </div>
                  <output className="setup-current" aria-label={`Current ${setupLabel(name)}`}>{setupCurrentValue(display(field.value,definition?.unit))}</output>
                  <div className="setup-action">
                    {kind==='status' ? <span className="setup-status">Read Only</span>
                      : kind==='routing' ? <button disabled={!connected} onClick={openOutputs}>Open Outputs</button>
                      : <SettingEditor key={`${identity}:${sessionId}:${name}`} name={name} scope="System" field={field} definition={definition}
                        disabled={!connected||!writeEnabled||busy} expectedSession={sessionId} action={action}/>}
                  </div>
                </div>;
              })}
            </div>
          </section>)}
        </article>)}
      </div>
    </div>
  </section>;
}
