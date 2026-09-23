import type { ProcessorSkin } from './skin-registry';

export function ModelMeterPanel({ skin }: { skin: ProcessorSkin }) {
  return <section className="model-meterstrip" aria-label={`${skin.productName} meter layout`}>
    <div className="model-meter-panel">
      {skin.meterGroups.map(group => <div className="model-meter-group" key={group}>
        <strong>{group}</strong>
        <div className="model-meter-wells" aria-hidden="true">
          {Array.from({ length: group.includes('Reduction') ? 5 : 2 }, (_, index) =>
            <span key={index}/>)}
        </div>
      </div>)}
    </div>
  </section>;
}
