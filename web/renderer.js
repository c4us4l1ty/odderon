import { vertex, sceneFragment, strokeVertex, strokeFragment } from './shaders.js';
import { renderSize } from './resolution.js';

function program(gl, vert, frag) {
  function shader(type, source) { const s=gl.createShader(type); gl.shaderSource(s,source); gl.compileShader(s); if (!gl.getShaderParameter(s,gl.COMPILE_STATUS)) throw Error(gl.getShaderInfoLog(s)); return s; }
  const v=shader(gl.VERTEX_SHADER,vert), f=shader(gl.FRAGMENT_SHADER,frag), p=gl.createProgram();
  gl.attachShader(p,v); gl.attachShader(p,f); gl.linkProgram(p); gl.deleteShader(v); gl.deleteShader(f);
  if(!gl.getProgramParameter(p,gl.LINK_STATUS)) throw Error(gl.getProgramInfoLog(p)); return p;
}
export class Renderer {
  constructor(canvas) {
    this.canvas=canvas; this.gl=canvas.getContext('webgl2',{alpha:false,antialias:false,depth:false,preserveDrawingBuffer:true,powerPreference:'low-power'});
    if(!this.gl) throw Error('WebGL2 is unavailable. Try a browser with WebGL2 enabled.');
    this.init();
  }
  init() {
    const gl=this.gl; this.scene=program(gl,vertex,sceneFragment);this.strokes=program(gl,strokeVertex,strokeFragment);
    this.fbo=gl.createFramebuffer();this.accumFbo=gl.createFramebuffer();this.textures=[gl.createTexture(),gl.createTexture()];this.accum=gl.createTexture();this.vao=gl.createVertexArray();gl.bindVertexArray(this.vao);this.width=0;this.height=0;this.batchIndex=-1;this.strokeCount=0;
    for(const texture of [...this.textures,this.accum]){gl.bindTexture(gl.TEXTURE_2D,texture);gl.texParameteri(gl.TEXTURE_2D,gl.TEXTURE_MIN_FILTER,gl.LINEAR);gl.texParameteri(gl.TEXTURE_2D,gl.TEXTURE_MAG_FILTER,gl.LINEAR);gl.texParameteri(gl.TEXTURE_2D,gl.TEXTURE_WRAP_S,gl.CLAMP_TO_EDGE);gl.texParameteri(gl.TEXTURE_2D,gl.TEXTURE_WRAP_T,gl.CLAMP_TO_EDGE);}
  }
  size(w,h){
    if(w===this.width && h===this.height)return;
    const gl=this.gl;this.width=w;this.height=h;this.canvas.width=w;this.canvas.height=h;gl.bindFramebuffer(gl.FRAMEBUFFER,this.fbo);
    this.textures.forEach((t,i)=>{gl.bindTexture(gl.TEXTURE_2D,t);gl.texImage2D(gl.TEXTURE_2D,0,gl.RGBA8,w,h,0,gl.RGBA,gl.UNSIGNED_BYTE,null);gl.framebufferTexture2D(gl.FRAMEBUFFER,gl.COLOR_ATTACHMENT0+i,gl.TEXTURE_2D,t,0);});
    gl.drawBuffers([gl.COLOR_ATTACHMENT0,gl.COLOR_ATTACHMENT1]);if(gl.checkFramebufferStatus(gl.FRAMEBUFFER)!==gl.FRAMEBUFFER_COMPLETE)throw Error('Unable to allocate rendering buffer.');
    gl.bindFramebuffer(gl.FRAMEBUFFER,this.accumFbo);gl.bindTexture(gl.TEXTURE_2D,this.accum);gl.texImage2D(gl.TEXTURE_2D,0,gl.RGBA8,w,h,0,gl.RGBA,gl.UNSIGNED_BYTE,null);gl.framebufferTexture2D(gl.FRAMEBUFFER,gl.COLOR_ATTACHMENT0,gl.TEXTURE_2D,this.accum,0);gl.drawBuffers([gl.COLOR_ATTACHMENT0]);if(gl.checkFramebufferStatus(gl.FRAMEBUFFER)!==gl.FRAMEBUFFER_COMPLETE)throw Error('Unable to allocate accumulation buffer.');
  }
  start(model, quality) {
    const gl=this.gl, rect=this.canvas.getBoundingClientRect(); if(rect.width<1||rect.height<1)return false;
    const [width,height]=renderSize(rect.width,rect.height,window.devicePixelRatio,quality);this.size(width,height);
    const w=this.width,h=this.height; gl.viewport(0,0,w,h);gl.bindVertexArray(this.vao);gl.bindFramebuffer(gl.FRAMEBUFFER,this.fbo);gl.drawBuffers([gl.COLOR_ATTACHMENT0,gl.COLOR_ATTACHMENT1]);gl.useProgram(this.scene);
    const set=(name,method,...args)=>gl[method](gl.getUniformLocation(this.scene,name),...args);
    set('uResolution','uniform2f',w,h);set('uEarth','uniform4fv',model.earth);set('uMoon','uniform4fv',model.moon);
    set('uVehicle','uniform1f',model.vehicle?1:0);set('uRight','uniform3fv',model.right);set('uUp','uniform3fv',model.up);set('uView','uniform3fv',model.view);set('uEarthLight','uniform3fv',model.earthLight);set('uMoonLight','uniform3fv',model.moonLight);
    set('uEarthOrientation','uniformMatrix3fv',false,model.earthOrientation);set('uMoonOrientation','uniformMatrix3fv',false,model.moonOrientation);
    gl.drawArrays(gl.TRIANGLES,0,3);
    // Seed accumulation once from the G-buffer; subsequent batches never clear it.
    gl.bindFramebuffer(gl.READ_FRAMEBUFFER,this.fbo);gl.readBuffer(gl.COLOR_ATTACHMENT0);gl.bindFramebuffer(gl.DRAW_FRAMEBUFFER,this.accumFbo);gl.blitFramebuffer(0,0,w,h,0,0,w,h,gl.COLOR_BUFFER_BIT,gl.NEAREST);
    this.batchIndex=-1;this.strokeCount=0;this.batch(0);return true;
  }
  batch(index){
    if(index!==this.batchIndex+1 || index<0 || index>15)throw Error('Invalid progressive stroke batch.');
    const gl=this.gl,w=this.width,h=this.height,count=index===0?60_000:96_000;
    gl.bindFramebuffer(gl.FRAMEBUFFER,this.accumFbo);gl.drawBuffers([gl.COLOR_ATTACHMENT0]);gl.viewport(0,0,w,h);gl.bindVertexArray(this.vao);gl.useProgram(this.strokes);
    this.textures.forEach((t,i)=>{gl.activeTexture(gl.TEXTURE0+i);gl.bindTexture(gl.TEXTURE_2D,t);gl.uniform1i(gl.getUniformLocation(this.strokes,i?'uNormal':'uColor'),i);});
    gl.uniform2f(gl.getUniformLocation(this.strokes,'uResolution'),w,h);gl.uniform1i(gl.getUniformLocation(this.strokes,'uBatch'),index);
    gl.enable(gl.BLEND);gl.blendFunc(gl.SRC_ALPHA,gl.ONE_MINUS_SRC_ALPHA);gl.drawArraysInstanced(gl.TRIANGLES,0,6,count);gl.disable(gl.BLEND);
    gl.bindFramebuffer(gl.READ_FRAMEBUFFER,this.accumFbo);gl.readBuffer(gl.COLOR_ATTACHMENT0);gl.bindFramebuffer(gl.DRAW_FRAMEBUFFER,null);gl.blitFramebuffer(0,0,w,h,0,0,w,h,gl.COLOR_BUFFER_BIT,gl.NEAREST);gl.bindFramebuffer(gl.FRAMEBUFFER,null);
    this.batchIndex=index;this.strokeCount+=count;
  }
}
