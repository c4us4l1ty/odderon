import { vertex, sceneFragment, paintFragment } from './shaders.js';
import { renderSize } from './resolution.js';

function program(gl, frag) {
  function shader(type, source) { const s=gl.createShader(type); gl.shaderSource(s,source); gl.compileShader(s); if (!gl.getShaderParameter(s,gl.COMPILE_STATUS)) throw Error(gl.getShaderInfoLog(s)); return s; }
  const v=shader(gl.VERTEX_SHADER,vertex), f=shader(gl.FRAGMENT_SHADER,frag), p=gl.createProgram();
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
    const gl=this.gl; this.scene=program(gl,sceneFragment); this.paint=program(gl,paintFragment); this.fbo=gl.createFramebuffer(); this.textures=[gl.createTexture(),gl.createTexture()]; this.vao=gl.createVertexArray(); gl.bindVertexArray(this.vao); this.width=0; this.height=0;
    for(const texture of this.textures){gl.bindTexture(gl.TEXTURE_2D,texture);gl.texParameteri(gl.TEXTURE_2D,gl.TEXTURE_MIN_FILTER,gl.LINEAR);gl.texParameteri(gl.TEXTURE_2D,gl.TEXTURE_MAG_FILTER,gl.LINEAR);gl.texParameteri(gl.TEXTURE_2D,gl.TEXTURE_WRAP_S,gl.CLAMP_TO_EDGE);gl.texParameteri(gl.TEXTURE_2D,gl.TEXTURE_WRAP_T,gl.CLAMP_TO_EDGE);}
  }
  size(w,h){
    if(w===this.width && h===this.height)return;
    const gl=this.gl;this.width=w;this.height=h;this.canvas.width=w;this.canvas.height=h;gl.bindFramebuffer(gl.FRAMEBUFFER,this.fbo);
    this.textures.forEach((t,i)=>{gl.bindTexture(gl.TEXTURE_2D,t);gl.texImage2D(gl.TEXTURE_2D,0,gl.RGBA8,w,h,0,gl.RGBA,gl.UNSIGNED_BYTE,null);gl.framebufferTexture2D(gl.FRAMEBUFFER,gl.COLOR_ATTACHMENT0+i,gl.TEXTURE_2D,t,0);});
    gl.drawBuffers([gl.COLOR_ATTACHMENT0,gl.COLOR_ATTACHMENT1]);if(gl.checkFramebufferStatus(gl.FRAMEBUFFER)!==gl.FRAMEBUFFER_COMPLETE)throw Error('Unable to allocate rendering buffer.');
  }
  draw(model, quality) {
    const gl=this.gl, rect=this.canvas.getBoundingClientRect(); if(rect.width<1||rect.height<1)return;
    // Bound pixel cost and memory, while retaining half linear resolution in motion.
    const [width,height]=renderSize(rect.width,rect.height,window.devicePixelRatio,quality);
    this.size(width,height);
    const w=this.width,h=this.height; gl.viewport(0,0,w,h);gl.bindVertexArray(this.vao);gl.bindFramebuffer(gl.FRAMEBUFFER,this.fbo);gl.drawBuffers([gl.COLOR_ATTACHMENT0,gl.COLOR_ATTACHMENT1]);gl.useProgram(this.scene);
    const set=(name,method,...args)=>gl[method](gl.getUniformLocation(this.scene,name),...args);
    set('uResolution','uniform2f',w,h);set('uEarth','uniform4fv',model.earth);set('uMoon','uniform4fv',model.moon);
    set('uVehicle','uniform1f',model.vehicle?1:0);set('uRight','uniform3fv',model.right);set('uUp','uniform3fv',model.up);set('uView','uniform3fv',model.view);set('uEarthLight','uniform3fv',model.earthLight);set('uMoonLight','uniform3fv',model.moonLight);
    set('uEarthOrientation','uniformMatrix3fv',false,model.earthOrientation);set('uMoonOrientation','uniformMatrix3fv',false,model.moonOrientation);
    gl.drawArrays(gl.TRIANGLES,0,3);
    gl.bindFramebuffer(gl.FRAMEBUFFER,null);gl.drawBuffers([gl.BACK]);gl.useProgram(this.paint);
    this.textures.forEach((t,i)=>{gl.activeTexture(gl.TEXTURE0+i);gl.bindTexture(gl.TEXTURE_2D,t);gl.uniform1i(gl.getUniformLocation(this.paint,i?'uNormal':'uColor'),i);});
    gl.uniform2f(gl.getUniformLocation(this.paint,'uResolution'),w,h);gl.uniform1f(gl.getUniformLocation(this.paint,'uQuality'),quality==='final'?1:0);gl.drawArrays(gl.TRIANGLES,0,3);
  }
}
