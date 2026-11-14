#!/usr/bin/env python3
"""Independent point-Earth tug impact experiment; requires NumPy and SciPy.

Uses a fixed 200 km circular start, fixed inertial 3120 m/s burn centered at
1600 s, and co-rotating drag. Intentionally uses zero density above 86 km,
neglects J2 and third bodies, and clamps density below the surface only to
locate the impact event. This is not the production atmosphere or an exact
impact-time oracle: it demonstrates how the specified scenario can fail.
"""
import numpy as np, math
from scipy.integrate import solve_ivp
mu=3.986004418e14;R=6378137.;rp=R+200e3;v=math.sqrt(mu/rp);omega=v/rp;F=18000;ex=450*9.80665;m0=18000;dv=3120;dur=m0*(-math.expm1(-dv/ex))*ex/F;t0=1600.;tstart=t0-dur/2;tend=t0+dur/2;u=np.array([-math.sin(omega*t0),math.cos(omega*t0),0])
def dens(h):
 if h>=200e3:return 0
 if h>=86e3:return 0 # underestimate high atmosphere
 H=[0,11000,20000,32000,47000,51000,71000];T=[288.15,216.65,216.65,228.65,270.65,270.65,214.65];P=[101325,22632.06,5474.889,868.0187,110.9063,66.93887,3.956420];L=[-.0065,0,.001,.0028,0,-.0028,-.002];h0=6356766*h/(6356766+h);i=max(i for i in range(7) if H[i]<=h0);dt=h0-H[i];t=T[i]+L[i]*dt;p=P[i]*math.exp(-9.80665*dt/(287.05287*T[i])) if L[i]==0 else P[i]*(T[i]/t)**(9.80665/(287.05287*L[i]));return p/(287.05287*t)
def f(t,y,active):
 r=y[:3];v=y[3:6];dist=np.linalg.norm(r);relative=v-np.cross([0,0,7.292115e-5],r);rho=dens(max(0,dist-R));drag=-.5*rho*2.2*4/y[6]*np.linalg.norm(relative)*relative;return np.r_[v,-mu*r/dist**3+drag+(F*u/y[6] if active else 0),-F/ex if active else 0]
def impact(t,y):return np.linalg.norm(y[:3])-R
impact.terminal=True;impact.direction=-1
y=np.array([rp,0,0,0,v,0,m0],float)
for a,b,act in [(0,tstart,False),(tstart,tend,True),(tend,20000,False)]:
 sol=solve_ivp(lambda t,y:f(t,y,act),[a,b],y,events=impact,rtol=1e-9,atol=1e-7,max_step=5);y=sol.y[:,-1];print('segment',a,b,'stop',sol.t[-1], 'alt',np.linalg.norm(y[:3])-R,'mass',y[-1]);
 for t in [1200,1400,1600,1800,2000,2200,2400,2600]:
  if t>=a and t<=sol.t[-1]:
   idx=np.searchsorted(sol.t,t);q=sol.y[:,min(idx,len(sol.t)-1)];r=np.linalg.norm(q[:3]);vv=q[3:6];h=np.linalg.norm(np.cross(q[:3],vv));E=np.dot(vv,vv)/2-mu/r;aa=-mu/(2*E);e=math.sqrt(max(0,1+2*E*h*h/mu**2));print(t,'alt',r-R,'apo km',aa*(1+e)/1000,'m',q[-1])
 if sol.status: break
