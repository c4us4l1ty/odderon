// Demand scheduler: one outstanding frame at most; progressive work is finite.
export class Governor {
  constructor(draw, clock = window) { this.draw=draw;this.clock=clock;this.frame=null;this.timer=null;this.dragging=false;this.suspended=false;this.stage=null;this.frameCount=0; }
  invalidate(world=true) {
    if(!this.dragging && world)this.cancelTimer();
    if(world)this.stage=0;
    if(this.suspended||this.frame!==null)return;
    this.frame=this.clock.requestAnimationFrame(()=>this.tick());
  }
  tick(){
    this.frame=null;if(this.suspended)return;this.frameCount++;
    if(this.dragging){this.stage=null;this.draw('interactive',0);return;}
    if(this.stage===null){this.draw('overlay',null);return;}
    const batch=this.stage;
    const keepGoing=this.draw('final',batch)!==false;
    // A draw callback may suspend, restart, or otherwise invalidate work.
    if(this.suspended||!keepGoing){this.stage=null;return;}
    if(this.stage===batch)this.stage=batch===15?null:batch+1;
    if(this.stage!==null && this.frame===null)this.frame=this.clock.requestAnimationFrame(()=>this.tick());
  }
  begin(){this.dragging=true;this.stage=null;this.cancelTimer();if(this.frame!==null){this.clock.cancelAnimationFrame(this.frame);this.frame=null;}}
  move(){if(this.dragging)this.invalidate(true);}
  end(){
    if(!this.dragging)return;this.dragging=false;
    if(this.frame!==null){this.clock.cancelAnimationFrame(this.frame);this.frame=null;}
    this.stage=null;this.cancelTimer();
    this.timer=this.clock.setTimeout(()=>{this.timer=null;this.invalidate(true);},50);
  }
  cancelTimer(){if(this.timer!==null){this.clock.clearTimeout(this.timer);this.timer=null;}}
  suspend(){this.suspended=true;this.dragging=false;this.stage=null;this.cancelTimer();if(this.frame!==null)this.clock.cancelAnimationFrame(this.frame);this.frame=null;}
  resume(){this.suspended=false;this.invalidate(true);}
}
