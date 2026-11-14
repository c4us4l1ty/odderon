import { vertex, sceneFragment } from './shaders.js';
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
  init() { const gl=this.gl;this.scene=program(gl,vertex,sceneFragment);this.vao=gl.createVertexArray();this.width=0;this.height=0; }
  start(model, quality) {
    const gl=this.gl, rect=this.canvas.getBoundingClientRect(); if(rect.width<1||rect.height<1)return false;
    const [width,height]=renderSize(rect.width,rect.height,window.devicePixelRatio,quality);
    if(width!==this.width||height!==this.height){this.width=width;this.height=height;this.canvas.width=width;this.canvas.height=height;}
    gl.bindFramebuffer(gl.FRAMEBUFFER,null);gl.viewport(0,0,width,height);gl.bindVertexArray(this.vao);gl.useProgram(this.scene);
    const set=(name,method,...args)=>gl[method](gl.getUniformLocation(this.scene,name),...args);
    set('uResolution','uniform2f',width,height);set('uEarth','uniform4fv',model.earth);set('uMoon','uniform4fv',model.moon);
    set('uVehicle','uniform1f',model.vehicle?1:0);set('uCraft','uniform3fv',model.craftProjection);set('uCraftLight','uniform3fv',model.craftLight);set('uEarthShine','uniform3fv',model.earthShine.direction);set('uMoonShine','uniform3fv',model.moonShine.direction);set('uBounce','uniform2f',model.earthShine.strength,model.moonShine.strength);set('uRight','uniform3fv',model.right);set('uUp','uniform3fv',model.up);set('uView','uniform3fv',model.view);set('uEarthLight','uniform3fv',model.earthLight);set('uMoonLight','uniform3fv',model.moonLight);
    set('uEarthOrientation','uniformMatrix3fv',false,model.earthOrientation);set('uMoonOrientation','uniformMatrix3fv',false,model.moonOrientation);
    gl.drawArrays(gl.TRIANGLES,0,3);return true;
  }
}
