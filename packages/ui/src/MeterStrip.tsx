import { useEffect, useMemo, useRef } from 'react';
import { apiPath } from './api';
import {
  METER_PALETTE,
  MeterMotion,
  meterCanvasWidth,
  meterChannelLayout,
  meterGroupWidth,
  REFERENCE_METERS,
  visibleGroups,
  type MeterGroup,
  type MeterModel,
  type MeterView,
  type SettingValues,
} from './meters';

const REFERENCE_WIDTH=1100;
const REFERENCE_HEIGHT=242;
// Every caption under the bars (lane, channel and unit) shares one baseline
// and size; gate lamps share one row below it.
const LABEL_FONT='500 8px "SF Compact Text", -apple-system, sans-serif';
const LABEL_BASELINE=226;
const GATE_BASELINE=239;

function titleFor(group: MeterGroup): string {
  return group.name
    .replace('FM HF Enhance','FM HF')
    .replace('HD HF Enhance','HD HF')
    .replace('FM Loudness','Loudness')
    .replace('Composite','Comp')
    .replace('HD Loudness','Loudness');
}

function ticksFor(group: MeterGroup): [number,string][] {
  if(group.ticks)return group.ticks;
  if(group.name==='Input')return ['0','3','6','9','12','15','18','24','30','36'].map((label,index)=>[index/9,label]);
  if(group.kind==='level'&&group.name.endsWith('Output'))return ['3','0','3','6','9','12','15','18','24','30'].map((label,index)=>[index/9,label]);
  if(group.kind==='composite')return ['125','100','75','50','25','0'].map((label,index)=>[index/5,label]);
  if(group.name.includes('HF Enhance'))return ['10','9','8','7','6','5','4','3','2','1'].map((label,index)=>[index/9,label]);
  if(group.name==='HD Limiting')return ['0','2','4','6','8','10','12'].map((label,index)=>[index/6,label]);
  if(group.name==='Loudness GR')return ['0','1','2','3','4','5','6','7','8','9','10'].map((label,index)=>[index/10,label]);
  if(group.kind==='loudness')return ['6','3','0','3','6','9','12','15'].map((label,index)=>[index/7,label]);
  return ['0','3','6','9','12','15','18','24'].map((label,index)=>[index/7,label]);
}

function unitFor(group: MeterGroup): string {
  if(group.name==='Loudness GR')return 'dB';
  if(group.name==='HD Loudness')return 'LU/Lk';
  return '';
}

/**
 * The original client's loudness bar (bitmap 397, 16 x 181) is amber in its
 * top 33 rows and green below, drawn over a 162-pixel control: the top 33/162
 * of the travel is amber. Its value table is the 'loudness' curve.
 */
const LOUDNESS_AMBER_FROM_TOP=33/162;

const GATE_LED_OFF='#4A2224';

function drawGateLed(context:CanvasRenderingContext2D,x:number,y:number,lit:boolean,palette:MeterPalette){
 context.save();
 if(lit){context.shadowColor=palette.red;context.shadowBlur=5;}
 context.fillStyle=lit?palette.red:GATE_LED_OFF;
 context.beginPath();context.arc(x,y,2.5,0,Math.PI*2);context.fill();
 context.restore();
}

function levelColor(percent:number,palette:MeterPalette=METER_PALETTE): string {
  if(percent>=92)return palette.red;
  if(percent>=78)return palette.amber;
  return palette.green;
}

type MeterPalette={[K in keyof typeof METER_PALETTE]:string};

/** The meter panel of the Figma Meters frame (150:2690): flat panel, no group grooves. */
const FIGMA_METER_PALETTE:MeterPalette={
 ...METER_PALETTE,
 panelTop:'#292929',panelBottom:'#292929',well:'#1c1c1c',separator:'#292929',separatorHighlight:'#292929',
 ticks:'#8a8a8a',label:'#d6d6d6',
};

export function MeterStrip({connected,identity,view,onLiveChange,model=REFERENCE_METERS,values,theme='instrument'}:{
  connected:boolean;
  identity:string;
  view:MeterView;
  onLiveChange?:(live:boolean)=>void;
  model?:MeterModel;
  values?:SettingValues;
  /** 'figma': the Meters window of the Figma design, the same for every model. */
  theme?:'instrument'|'figma';
}) {
 const palette=theme==='figma'?FIGMA_METER_PALETTE:METER_PALETTE;
 const groups=useMemo(()=>visibleGroups(view,model,values),[view,model,values]);
 const minimumCanvasWidth=meterCanvasWidth(groups);
 const canvas=useRef<HTMLCanvasElement>(null);
 const motion=useRef(new MeterMotion(model));
 useEffect(()=>{motion.current.setModel(model);},[model]);
 useEffect(()=>{
  motion.current.clear();onLiveChange?.(false);
  if(!connected)return;
  const source=new EventSource(apiPath('meters'));
  source.onmessage=event=>{try{const meter=JSON.parse(event.data);if(meter.live)motion.current.push(meter.values,performance.now(),meter.peaks);else motion.current.clear();}catch{motion.current.clear();}};
  source.onerror=()=>motion.current.clear();
  return()=>{source.close();motion.current.clear();onLiveChange?.(false);};
 },[connected,identity,onLiveChange]);
 useEffect(()=>{
  let animation=0,lastLive=false;
  const draw=(now:number)=>{
   const element=canvas.current,context=element?.getContext('2d');
   if(!element||!context)return;
   const values=connected?motion.current.sample(now):null;
   const peaks=motion.current.peakValues();
   const active=!!values;
   if(active!==lastLive){lastLive=active;onLiveChange?.(active);}
   const width=element.clientWidth,height=element.clientHeight,ratio=devicePixelRatio||1;
   if(element.width!==Math.round(width*ratio)||element.height!==Math.round(height*ratio)){
    element.width=Math.round(width*ratio);element.height=Math.round(height*ratio);
   }
   context.setTransform(ratio,0,0,ratio,0,0);context.clearRect(0,0,width,height);
   // The Figma window draws the meters at their real size and spacing, never
   // scaled or spread apart: a narrower window scrolls.
   if(theme==='figma'){
    context.fillStyle=palette.panelTop;context.fillRect(0,0,width,height);
   }else{
    context.setTransform(ratio*width/minimumCanvasWidth,0,0,ratio*height/REFERENCE_HEIGHT,0,0);
    const panelGradient=context.createLinearGradient(0,0,0,REFERENCE_HEIGHT);
    panelGradient.addColorStop(0,palette.panelTop);panelGradient.addColorStop(1,palette.panelBottom);
    context.fillStyle=panelGradient;context.fillRect(0,0,minimumCanvasWidth,REFERENCE_HEIGHT);
   }
   context.imageSmoothingEnabled=false;
   let x=0;
   groups.forEach((group,index)=>{
    const groupWidth=meterGroupWidth(group),top=46.096,bottom=214.096,meterHeight=bottom-top;
    context.font=theme==='figma'?'500 10.5px -apple-system, "SF Pro Text", sans-serif':'600 10px "SF Compact Text", -apple-system, sans-serif';
    context.textAlign='center';context.fillStyle=palette.label;
    if(group.name==='Loudness GR'){
     context.fillText('Loudness',x+groupWidth/2,22.5,groupWidth-8);
     context.fillText('Gr',x+groupWidth/2,33,groupWidth-8);
    }else{
     const title=titleFor(group);
     // A title wider than its group wraps onto two lines, as "Loudness Gr" does.
     const words=title.split(' ');
     if(words.length>1&&context.measureText(title).width>groupWidth-8){
      const split=Math.ceil(words.length/2);
      context.fillText(words.slice(0,split).join(' '),x+groupWidth/2,22.5,groupWidth-4);
      context.fillText(words.slice(split).join(' '),x+groupWidth/2,33,groupWidth-4);
     }else context.fillText(title,x+groupWidth/2,26.2,groupWidth-8);
    }
    const channels=meterChannelLayout(group);
    const contentStart=x+group.contentOffset;
    const firstWell=contentStart+(channels[0]?.wellLeft||0);
    context.font='500 7px "SF Compact Text", -apple-system, sans-serif';
    context.textAlign='right';context.fillStyle=palette.ticks;
    ticksFor(group).forEach(([position,label],tickIndex)=>{
     const y=top+2.5+position*(meterHeight-5);
     const signedOutput=group.kind==='level'&&group.name.endsWith('Output')&&tickIndex===0;
     context.fillText(signedOutput?`+${label}`:label,firstWell-5,y+2);
     context.fillStyle=palette.ticks;context.fillRect(firstWell-3,y,2,.55);
    });
    channels.forEach(channel=>{
     const wellX=contentStart+channel.wellLeft;
     context.fillStyle=palette.well;
     const wellGradient=context.createLinearGradient(0,top,0,bottom);
     wellGradient.addColorStop(0,theme==='figma'?palette.well:'rgba(20,20,20,.8)');wellGradient.addColorStop(1,palette.well);
     context.fillStyle=wellGradient;
     context.beginPath();context.roundRect(wellX,top,channel.wellWidth,meterHeight,2.3913);context.fill();
     channel.lanes.forEach(lane=>{
      const laneX=contentStart+lane.left;
      if(values){
       const value=Math.max(0,Math.min(100,values[lane.id]??0));
       const peak=Math.max(value,Math.min(100,peaks[lane.id]??value));
       const reduction=group.kind==='reduction';
       const activeTop=top+2.5,activeBottom=bottom-2.5,activeHeight=activeBottom-activeTop;
       const barHeight=value/100*activeHeight;
       const barY=reduction?activeTop:activeBottom-barHeight;
       let color:string=group.kind==='reduction'?palette.blue:palette.amber;
       if(group.kind==='level'){
        const gradient=context.createLinearGradient(0,activeTop,0,activeBottom);
        gradient.addColorStop(0,palette.red);gradient.addColorStop(.07362,palette.red);
        gradient.addColorStop(.07363,'#F8D448');gradient.addColorStop(.1853,'#F8D448');
        gradient.addColorStop(.3714,palette.green);gradient.addColorStop(1,palette.green);
        context.fillStyle=gradient;
       } else if(group.kind==='loudness'){
        const amberBottom=activeTop+LOUDNESS_AMBER_FROM_TOP*activeHeight;
        const gradient=context.createLinearGradient(0,activeTop,0,activeBottom);
        gradient.addColorStop(0,palette.amber);gradient.addColorStop(LOUDNESS_AMBER_FROM_TOP,palette.amber);
        gradient.addColorStop(LOUDNESS_AMBER_FROM_TOP,palette.green);gradient.addColorStop(1,palette.green);
        context.fillStyle=gradient;
        color=activeBottom-peak/100*activeHeight<amberBottom?palette.amber:palette.green;
       } else context.fillStyle=color;
       context.fillRect(laneX,barY,lane.width,barHeight);
       const peakY=reduction?activeTop+peak/100*activeHeight:activeBottom-peak/100*activeHeight;
       if(group.kind==='level')color=levelColor(peak,palette);
       context.fillStyle=color;context.fillRect(laneX,peakY-.5,lane.width,1);
      }
      if(lane.label){
       context.font=LABEL_FONT;context.textAlign='center';context.fillStyle=palette.ticks;
       context.fillText(lane.label,laneX+lane.width/2,LABEL_BASELINE);
      }
     });
     if(channel.label){
      context.font=LABEL_FONT;context.textAlign='center';context.fillStyle=palette.ticks;
      context.fillText(channel.label,contentStart+channel.center,LABEL_BASELINE);
     }
    });
    // Gate lamps: a red LED, lit while the firmware reports the gate (the
    // original client's lamp). AGC keeps the Figma frame's vertical GATED
    // under its LED; other groups carry LED and label under their lanes.
    if(group.gate){
     const lit=!!values&&motion.current.gated(group.gate.id);
     context.textAlign='center';context.fillStyle=palette.ticks;
     if(group.gate.vertical){
      drawGateLed(context,x+54.75,90,lit,palette);
      context.font='600 9px "SF Compact Text", -apple-system, sans-serif';
      [...group.gate.label].forEach((letter,letterIndex)=>context.fillText(letter,x+54.75,108+letterIndex*12));
     }else{
      context.font='600 7px "SF Compact Text", -apple-system, sans-serif';
      const center=contentStart+(channels.at(-1)!.wellLeft+channels.at(-1)!.wellWidth)/2;
      const textWidth=context.measureText(group.gate.label).width;
      drawGateLed(context,center-(textWidth+7)/2+2.5,GATE_BASELINE-2.4,lit,palette);
      context.textAlign='left';context.fillText(group.gate.label,center-(textWidth+7)/2+7,GATE_BASELINE);
     }
    }
    context.font=LABEL_FONT;context.fillStyle=palette.ticks;context.textAlign='center';
    const unit=unitFor(group);
    // Centred under the bars, not the group (whose left part holds the scale).
    if(unit)context.fillText(unit,contentStart+(channels[0].wellLeft+channels.at(-1)!.wellLeft+channels.at(-1)!.wellWidth)/2,LABEL_BASELINE);
    if(index<groups.length-1){
     const divider=x+groupWidth;
     context.fillStyle=palette.separator;context.fillRect(divider-1,0,1,REFERENCE_HEIGHT);
     context.fillStyle=palette.separatorHighlight;context.fillRect(divider,0,1,REFERENCE_HEIGHT);
    }
    x+=groupWidth;
   });
   animation=requestAnimationFrame(draw);
  };
  animation=requestAnimationFrame(draw);
  return()=>cancelAnimationFrame(animation);
 },[connected,identity,view,onLiveChange,groups,minimumCanvasWidth,palette,theme]);
 // The native Meters window opens wide enough for this model's meters.
 useEffect(()=>{
  if(theme!=='figma')return;
  const handler=(window as unknown as {webkit?:{messageHandlers?:Record<string,{postMessage:(body:unknown)=>void}>}}).webkit?.messageHandlers?.meterSize;
  const strip=canvas.current?.closest('.meterstrip');
  if(!handler||!strip)return;
  const style=getComputedStyle(strip);
  const padX=parseFloat(style.paddingLeft)+parseFloat(style.paddingRight);
  const padY=parseFloat(style.paddingTop)+parseFloat(style.paddingBottom);
  const top=strip.getBoundingClientRect().top+window.scrollY;
  handler.postMessage({width:Math.ceil(minimumCanvasWidth+padX),height:Math.ceil(top+REFERENCE_HEIGHT+padY)});
 },[theme,minimumCanvasWidth]);
 return <section className="meterstrip" aria-label="Live meter groups">
  <div className="meter-canvas-scroll">
   <canvas ref={canvas} style={theme==='figma'?{width:`max(100%, ${minimumCanvasWidth}px)`,height:`${REFERENCE_HEIGHT}px`,flex:'0 0 auto'}:{width:`${minimumCanvasWidth/REFERENCE_WIDTH*100}%`,minWidth:`${minimumCanvasWidth}px`}} role="img" aria-label={groups.map(group=>group.name).join(', ')}/>
  </div>
  <ul className="sr-only">{groups.map(group=><li key={group.name}>{group.name}: {group.channels.flatMap(channel=>channel.laneLabels || [channel.label]).join(', ')}</li>)}</ul>
 </section>;
}
