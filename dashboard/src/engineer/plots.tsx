import { useMemo, useState } from 'react'
import { ResponsiveContainer,LineChart,Line,XAxis,YAxis,Tooltip,CartesianGrid,ReferenceLine } from 'recharts'
import type { Sample,Standing } from './types'
import { num } from './types'

export function TrackPlot({samples=[],reference=[],cars=[]}:{samples?:Sample[];reference?:Sample[];cars?:Standing[]}){
 samples=samples.filter(s=>s.supplemental?.position_available!==false)
 reference=reference.filter(s=>s.supplemental?.position_available!==false)
 const [hover,setHover]=useState<number|null>(null)
 const points=[...samples,...reference,...cars];const xs=points.map(s=>s.x),zs=points.map(s=>s.z)
 const minX=Math.min(...xs),maxX=Math.max(...xs),minZ=Math.min(...zs),maxZ=Math.max(...zs)
 const scale=Math.min(560/Math.max(1,maxX-minX),300/Math.max(1,maxZ-minZ))
 const p=(s:{x:number;z:number})=>`${(s.x-minX)*scale+25},${325-(s.z-minZ)*scale}`
 if(!points.length)return <div className="eng-empty small">Track geometry appears after telemetry is recorded.</div>
 return <div className="eng-map"><svg viewBox="0 0 620 350" aria-label="Track positions and racing lines">
  <defs><pattern id="map-grid" width="25" height="25" patternUnits="userSpaceOnUse"><path d="M25 0H0V25" fill="none" stroke="#243142" strokeWidth=".5"/></pattern></defs><rect width="620" height="350" fill="url(#map-grid)"/>
  {reference.length>0&&<polyline points={reference.map(p).join(' ')} stroke="#a398ff" fill="none" strokeWidth="3" opacity=".7"/>}
  {samples.length>0&&<polyline points={samples.map(p).join(' ')} stroke="#55e1b5" fill="none" strokeWidth="2"/>}
  {cars.map(c=>{const [x,y]=p(c).split(',');return <g key={c.slot} onMouseEnter={()=>setHover(c.slot)} onMouseLeave={()=>setHover(null)}><circle cx={x} cy={y} r="6" fill={c.class.includes('GT')?'#55e1b5':'#ffb05d'} stroke="#0b1018" strokeWidth="2"/><text x={+x+10} y={+y+4} fill="#f1f5fb" fontSize="11">{hover===c.slot?c.driver:`P${c.position}`}</text></g>})}
 </svg><div className="eng-legend"><span className="mint">● Current</span><span className="purple">● Reference</span><span>Coordinates in metres · true aspect ratio</span></div></div>
}
export function TelemetryPlot({samples,reference=[],channel='speed_kmh',label='Speed · km/h'}:{samples:Sample[];reference?:Sample[];channel:keyof Sample|'delta_s';label?:string}){
 const data=useMemo(()=>{
  const ref=reference.filter(s=>s.lap_distance_m>=0);let i=0
  return samples.filter((_,j)=>j%Math.max(1,Math.floor(samples.length/1200))===0).map(s=>{
   while(i<ref.length-1&&ref[i+1].lap_distance_m<=s.lap_distance_m)i++
   const a=ref[i],b=ref[i+1]
   if(channel==='delta_s'){
    const span=b&&a?b.lap_distance_m-a.lap_distance_m:0
    const t=a&&b&&span>0&&span<150&&s.lap_distance_m>=a.lap_distance_m&&s.lap_distance_m<=b.lap_distance_m?a.game_s+(b.game_s-a.game_s)*(s.lap_distance_m-a.lap_distance_m)/span:null
    return{distance:s.lap_distance_m,current:t==null?null:(s.game_s-s.lap_start_s)-(t-a.lap_start_s),reference:t==null?null:0}
   }
   return{distance:s.lap_distance_m,current:typeof s[channel]==='number'?s[channel]:null,reference:a&&typeof a[channel]==='number'?a[channel]:null}
  })
 },[samples,reference,channel])
 return <div className="eng-trace"><div className="eng-label">{label}</div><ResponsiveContainer width="100%" height={170}><LineChart data={data} syncId="telemetry"><CartesianGrid stroke="#263143" strokeDasharray="3 6"/><XAxis dataKey="distance" type="number" domain={['dataMin','dataMax']} tick={{fontSize:10,fill:'#8b9bb2'}} tickFormatter={v=>`${Math.round(v)}m`}/><YAxis width={40} tick={{fontSize:10,fill:'#8b9bb2'}}/><Tooltip contentStyle={{background:'#111a27',border:'1px solid #33435a',fontSize:11}} labelFormatter={v=>`${num(Number(v),0)} metres`}/><Line dataKey="reference" stroke="#a398ff" dot={false} strokeWidth={1.5} isAnimationActive={false}/><Line dataKey="current" stroke="#55e1b5" dot={false} strokeWidth={1.6} isAnimationActive={false}/>{(channel==='steering'||channel==='delta_s')&&<ReferenceLine y={0} stroke="#7b8ca1"/>}</LineChart></ResponsiveContainer></div>
}
