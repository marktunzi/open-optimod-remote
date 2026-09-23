import { useEffect, useMemo, useRef } from 'react';
import {
  METER_PALETTE,
  MeterMotion,
  meterCanvasWidth,
  meterChannelLayout,
  meterGroupWidth,
  visibleGroups,
  type MeterGroup,
  type MeterView,
} from './meters';

const REFERENCE_WIDTH=1100;
const REFERENCE_HEIGHT=242;

function titleFor(group: MeterGroup): string {
  return group.name
    .replace('FM HF Enhance','FM HF')
    .replace('HD HF Enhance','HD HF')
    .replace('FM Loudness','Loudness')
    .replace('Composite','Comp')
    .replace('HD Loudness','Loudness');
}

function ticksFor(group: MeterGroup): [number,string][] {
  if(group.name==='Input')return ['0','3','6','9','12','15','18','24','30','36'].map((label,index)=>[index/9,label]);
  if(group.name==='FM Output'||group.name==='HD Output')return ['3','0','3','6','9','12','15','18','24','30'].map((label,index)=>[index/9,label]);
  if(group.name==='Composite')return ['125','100','75','50','25','0'].map((label,index)=>[index/5,label]);
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

function levelColor(percent:number): string {
  if(percent>=92)return METER_PALETTE.red;
  if(percent>=78)return METER_PALETTE.amber;
  return METER_PALETTE.green;
}

export function MeterStrip({connected,identity,view,onLiveChange}:{
  connected:boolean;
  identity:string;
  view:MeterView;
  onLiveChange?:(live:boolean)=>void;
}) {
 const groups=useMemo(()=>visibleGroups(view),[view]);
 const minimumCanvasWidth=meterCanvasWidth(groups);
 const canvas=useRef<HTMLCanvasElement>(null);
 const motion=useRef(new MeterMotion());
 useEffect(()=>{
  motion.current.clear();onLiveChange?.(false);
  if(!connected)return;
  const source=new EventSource('/api/meters');
  source.onmessage=event=>{try{const meter=JSON.parse(event.data);if(meter.live)motion.current.push(meter.values,performance.now());else motion.current.clear();}catch{motion.current.clear();}};
  source.onerror=()=>motion.current.clear();
  return()=>{source.close();motion.current.clear();onLiveChange?.(false);};
 },[connected,identity,onLiveChange]);
 useEffect(()=>{
  let animation=0,lastLive=false;
  const reduced=matchMedia('(prefers-reduced-motion: reduce)');
  const draw=(now:number)=>{
   const element=canvas.current,context=element?.getContext('2d');
   if(!element||!context)return;
   const values=connected?motion.current.sample(now,reduced.matches):null;
   const peaks=motion.current.peakValues();
   const active=!!values;
   if(active!==lastLive){lastLive=active;onLiveChange?.(active);}
   const width=element.clientWidth,height=element.clientHeight,ratio=devicePixelRatio||1;
   if(element.width!==Math.round(width*ratio)||element.height!==Math.round(height*ratio)){
    element.width=Math.round(width*ratio);element.height=Math.round(height*ratio);
   }
   context.setTransform(ratio,0,0,ratio,0,0);context.clearRect(0,0,width,height);
   const scaleX=width/minimumCanvasWidth,scaleY=height/REFERENCE_HEIGHT;
   context.setTransform(ratio*scaleX,0,0,ratio*scaleY,0,0);
   context.imageSmoothingEnabled=false;
   const panelGradient=context.createLinearGradient(0,0,0,REFERENCE_HEIGHT);
   panelGradient.addColorStop(0,METER_PALETTE.panelTop);panelGradient.addColorStop(1,METER_PALETTE.panelBottom);
   context.fillStyle=panelGradient;context.fillRect(0,0,minimumCanvasWidth,REFERENCE_HEIGHT);
   let x=0;
   groups.forEach((group,index)=>{
    const groupWidth=meterGroupWidth(group),top=46.096,bottom=214.096,meterHeight=bottom-top;
    context.font='600 10px "SF Compact Text", -apple-system, sans-serif';
    context.textAlign='center';context.fillStyle=METER_PALETTE.label;
    if(group.name==='Loudness GR'){
     context.fillText('Loudness',x+groupWidth/2,22.5,groupWidth-8);
     context.fillText('Gr',x+groupWidth/2,33,groupWidth-8);
    }else context.fillText(titleFor(group),x+groupWidth/2,26.2,groupWidth-8);
    const channels=meterChannelLayout(group);
    const contentStart=x+group.contentOffset;
    const firstWell=contentStart+(channels[0]?.wellLeft||0);
    context.font='500 7px "SF Compact Text", -apple-system, sans-serif';
    context.textAlign='right';context.fillStyle=METER_PALETTE.ticks;
    ticksFor(group).forEach(([position,label],tickIndex)=>{
     const y=top+2.5+position*(meterHeight-5);
     const signedOutput=(group.name==='FM Output'||group.name==='HD Output')&&tickIndex===0;
     context.fillText(signedOutput?`+${label}`:label,firstWell-5,y+2);
     context.fillStyle=METER_PALETTE.ticks;context.fillRect(firstWell-3,y,2,.55);
    });
    channels.forEach(channel=>{
     const wellX=contentStart+channel.wellLeft;
     context.fillStyle=METER_PALETTE.well;
     const wellGradient=context.createLinearGradient(0,top,0,bottom);
     wellGradient.addColorStop(0,'rgba(20,20,20,.8)');wellGradient.addColorStop(1,METER_PALETTE.well);
     context.fillStyle=wellGradient;
     context.beginPath();context.roundRect(wellX,top,channel.wellWidth,meterHeight,2.3913);context.fill();
     channel.lanes.forEach(lane=>{
      const laneX=contentStart+lane.left;
      if(values){
       const value=Math.max(0,Math.min(100,values[lane.id]));
       const peak=Math.max(value,Math.min(100,peaks[lane.id]??value));
       const reduction=group.kind==='reduction';
       const activeTop=top+2.5,activeBottom=bottom-2.5,activeHeight=activeBottom-activeTop;
       const barHeight=value/100*activeHeight;
       const barY=reduction?activeTop:activeBottom-barHeight;
       let color:string=group.kind==='reduction'?METER_PALETTE.blue:METER_PALETTE.amber;
       if(group.kind==='level'){
        const gradient=context.createLinearGradient(0,activeTop,0,activeBottom);
        gradient.addColorStop(0,METER_PALETTE.red);gradient.addColorStop(.07362,METER_PALETTE.red);
        gradient.addColorStop(.07363,'#F8D448');gradient.addColorStop(.1853,'#F8D448');
        gradient.addColorStop(.3714,METER_PALETTE.green);gradient.addColorStop(1,METER_PALETTE.green);
        context.fillStyle=gradient;
       } else context.fillStyle=color;
       context.fillRect(laneX,barY,lane.width,barHeight);
       const peakY=reduction?activeTop+peak/100*activeHeight:activeBottom-peak/100*activeHeight;
       if(group.kind==='level')color=levelColor(peak);
       context.fillStyle=color;context.fillRect(laneX,peakY-.5,lane.width,1);
      }
      if(lane.label){
       context.font='500 6px "SF Compact Text", -apple-system, sans-serif';context.textAlign='center';context.fillStyle=METER_PALETTE.ticks;
       context.fillText(lane.label,laneX+lane.width/2,224.5);
      }
     });
     if(channel.label){
      context.font='500 6px "SF Compact Text", -apple-system, sans-serif';context.textAlign='center';context.fillStyle=METER_PALETTE.ticks;
      context.fillText(channel.label,contentStart+channel.center,224.5);
     }
    });
    if(group.name==='AGC'){
     context.font='600 9px "SF Compact Text", -apple-system, sans-serif';context.fillStyle=METER_PALETTE.ticks;context.textAlign='center';
     ['G','A','T','E','D'].forEach((letter,letterIndex)=>context.fillText(letter,x+54.75,105+letterIndex*12));
    }
    context.font='500 6px "SF Compact Text", -apple-system, sans-serif';context.fillStyle=METER_PALETTE.ticks;context.textAlign='center';
    const unit=unitFor(group);
    if(unit)context.fillText(unit,x+groupWidth/2,232.5);
    if(index<groups.length-1){
     const divider=x+groupWidth;
     context.fillStyle=METER_PALETTE.separator;context.fillRect(divider-1,0,1,REFERENCE_HEIGHT);
     context.fillStyle=METER_PALETTE.separatorHighlight;context.fillRect(divider,0,1,REFERENCE_HEIGHT);
    }
    x+=groupWidth;
   });
   animation=requestAnimationFrame(draw);
  };
  animation=requestAnimationFrame(draw);
  return()=>cancelAnimationFrame(animation);
 },[connected,identity,view,onLiveChange,groups,minimumCanvasWidth]);
 return <section className="meterstrip" aria-label="Live meter groups">
  <div className="meter-canvas-scroll">
   <canvas ref={canvas} style={{width:`${minimumCanvasWidth/REFERENCE_WIDTH*100}%`,minWidth:`${minimumCanvasWidth}px`}} role="img" aria-label={groups.map(group=>group.name).join(', ')}/>
  </div>
  <ul className="sr-only">{groups.map(group=><li key={group.name}>{group.name}: {group.channels.flatMap(channel=>channel.laneLabels || [channel.label]).join(', ')}</li>)}</ul>
 </section>;
}
