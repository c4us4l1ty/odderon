// Demand scheduler: at most one pending frame, no animation while idle.
export class Governor {
  constructor(draw, clock = window) { this.draw=draw;this.clock=clock;this.frame=null;this.dragging=false;this.suspended=false;this.world=false;this.frameCount=0; }
  invalidate(world=true) {
    this.world ||= world;
    if(this.suspended||this.frame!==null)return;
    this.frame=this.clock.requestAnimationFrame(()=>this.tick());
  }
  tick(){
    this.frame=null;if(this.suspended)return;this.frameCount++;
    const world=this.world;this.world=false;
    this.draw(world?(this.dragging?'interactive':'final'):'overlay');
  }
  begin(){this.dragging=true;}
  move(){if(this.dragging)this.invalidate(true);}
  end(){if(!this.dragging)return;this.dragging=false;this.invalidate(true);}
  suspend(){this.suspended=true;this.dragging=false;this.world=false;if(this.frame!==null)this.clock.cancelAnimationFrame(this.frame);this.frame=null;}
  resume(){this.suspended=false;this.invalidate(true);}
}
