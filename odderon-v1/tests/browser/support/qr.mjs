import assert from 'node:assert/strict';
import jsQR from 'jsqr';

// Rebuild the QR matrix from the emitted vector rectangles and decode it with
// an independent decoder. This proves the printed code carries a payload that
// actually verifies, rather than trusting the encoder a second time.
export function decodeQrFromSvg(svg){
  const size=Number(svg.match(/data-qr-size="(\d+)"/)[1]);
  const quiet=Number(svg.match(/data-qr-quiet="(\d+)"/)[1]);
  const outer=size+2*quiet;
  const start=svg.indexOf('<g fill="#000000">');
  assert.ok(start>=0,'the QR must emit a dark-module group');
  // Close the group relative to its own start: the trajectory figure earlier
  // in the document has groups too.
  const group=svg.slice(start,svg.indexOf('</g>',start));
  const grid=Array.from({length:outer},()=>Array(outer).fill(false));
  for(const match of group.matchAll(/<rect x="(\d+)" y="(\d+)" width="(\d+)" height="1"\/>/g)){
    const [,x,y,width]=match.map(Number);
    for(let offset=0;offset<width;offset++)grid[y][x+offset]=true;
  }
  const scale=6,pixels=outer*scale;
  const data=new Uint8ClampedArray(pixels*pixels*4);
  for(let y=0;y<pixels;y++)for(let x=0;x<pixels;x++){
    const dark=grid[Math.floor(y/scale)][Math.floor(x/scale)];
    const at=(y*pixels+x)*4,value=dark?0:255;
    data[at]=value;data[at+1]=value;data[at+2]=value;data[at+3]=255;
  }
  const decoded=jsQR(data,pixels,pixels);
  assert.ok(decoded,'an independent decoder must read the emitted QR');
  return decoded.data;
}
