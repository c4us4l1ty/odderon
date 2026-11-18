import { CANVAS_MONO } from './glyphs.js';
// Reusable Canvas2D telemetry primitives. Callers must supply real computed
// geometry and labels; nothing here creates an orbit, apsis, or B-plane.
const finitePoint=p=>Array.isArray(p)&&p.length===2&&p.every(Number.isFinite);
export function polyline(ctx, points, {stroke='#bacfc0', dashed=false, width=1}={}) {
  ctx.save();ctx.beginPath();let started=false;
  for(const p of points){if(!finitePoint(p)){started=false;continue;}if(started)ctx.lineTo(...p);else{ctx.moveTo(...p);started=true;}}
  ctx.strokeStyle=stroke;ctx.lineWidth=width;ctx.setLineDash(dashed?[3,5]:[]);ctx.stroke();ctx.restore();
}
export function vector(ctx, from, to, {color='#e1bd78'}={}) {
  if(!finitePoint(from)||!finitePoint(to))return false;
  const dx=to[0]-from[0],dy=to[1]-from[1],length=Math.hypot(dx,dy);
  // An almost line-of-sight vector has no discernible screen direction.
  if(length<1)return false;
  const angle=Math.atan2(dy,dx),head=Math.min(8,length*.4);ctx.save();ctx.strokeStyle=color;ctx.fillStyle=color;ctx.lineWidth=1.5;
  ctx.beginPath();ctx.moveTo(...from);ctx.lineTo(...to);ctx.stroke();ctx.beginPath();ctx.moveTo(...to);ctx.lineTo(to[0]-head*Math.cos(angle-.45),to[1]-head*Math.sin(angle-.45));ctx.lineTo(to[0]-head*Math.cos(angle+.45),to[1]-head*Math.sin(angle+.45));ctx.closePath();ctx.fill();ctx.restore();return true;
}
// Apsis is a supplied screen position and label, not detected by drawing code.
export function apsis(ctx, point, label, {color='#d5bb80'}={}) {
  if(!finitePoint(point)||typeof label!=='string')return;
  ctx.save();ctx.strokeStyle=color;ctx.fillStyle=color;ctx.lineWidth=1;ctx.beginPath();ctx.arc(point[0],point[1],3,0,Math.PI*2);ctx.stroke();ctx.font=`10px ${CANVAS_MONO}`;ctx.fillText(label,point[0]+7,point[1]-7);ctx.restore();
}
// Diagram in caller-supplied pixel coordinates; target is optional. The caller
// supplies all scaling, B-plane basis, units and actual trajectory solutions.
export function bPlane(ctx, center, radius, {target=null, label='B-plane', color='#d5bb80'}={}) {
  if(!finitePoint(center)||!Number.isFinite(radius)||radius<=0||(target!==null&&!finitePoint(target)))return;
  const [x,y]=center;ctx.save();ctx.strokeStyle=color;ctx.fillStyle=color;ctx.lineWidth=1;
  for(const r of [radius*.5,radius]){ctx.beginPath();ctx.arc(x,y,r,0,Math.PI*2);ctx.stroke();}
  ctx.beginPath();ctx.moveTo(x-radius,y);ctx.lineTo(x+radius,y);ctx.moveTo(x,y-radius);ctx.lineTo(x,y+radius);ctx.stroke();
  if(target){ctx.beginPath();ctx.arc(target[0],target[1],3,0,Math.PI*2);ctx.fill();}
  ctx.font=`10px ${CANVAS_MONO}`;ctx.fillText(label,x-radius,y-radius-8);ctx.restore();
}

// A screen-space locator, not an enlargement of the physical object.
export function reticle(ctx, point, {color='#dce8d5'}={}) {
  if(!finitePoint(point))return;
  const [x,y]=point;ctx.save();ctx.strokeStyle=color;ctx.lineWidth=1;ctx.beginPath();ctx.arc(x,y,5,0,Math.PI*2);ctx.moveTo(x-8,y);ctx.lineTo(x-4,y);ctx.moveTo(x+4,y);ctx.lineTo(x+8,y);ctx.moveTo(x,y-8);ctx.lineTo(x,y-4);ctx.moveTo(x,y+4);ctx.lineTo(x,y+8);ctx.stroke();ctx.restore();
}
