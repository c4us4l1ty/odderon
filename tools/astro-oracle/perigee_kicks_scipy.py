#!/usr/bin/env python3
"""Point-Earth finite-burn feasibility experiment; requires NumPy and SciPy.

Not the production integrator. Constant inertial thrust aligned with the coast
velocity at each node, and two equal delta-v kicks at consecutive osculating
periapses; times and steering differ for other models. No Moon or atmosphere.
"""
import math
import numpy as np
from scipy.integrate import solve_ivp
mu=3.986004418e14; R=6378137.;r=R+2e5; v=math.sqrt(mu/r);m0=18000.; isp=450.; g=9.80665;F=18000.;dv=3120.
def eom(t,y,force):
 r=y[:3];v=y[3:6];m=y[6]; rr=np.linalg.norm(r); return np.r_[v,-mu*r/rr**3+force/m,-np.linalg.norm(force)/(isp*g)]
def solve(y,t,force=np.zeros(3), events=None):
 return solve_ivp(lambda t,y:eom(t,y,force),[0,t],y,rtol=2e-12,atol=1e-9,max_step=20,events=events)
def apo(y):
 r=y[:3];v=y[3:6];rr=np.linalg.norm(r);h=np.linalg.norm(np.cross(r,v)); en=np.dot(v,v)/2-mu/rr
 if en>=0:return float('inf')
 a=-mu/(2*en);e=math.sqrt(max(0,1+2*en*h*h/mu**2));return a*(1+e)/1e3
node=np.array([r,0,0,0,v,0,m0],float)
def burn(node,dv):
 m=node[6];dur=m*(-math.expm1(-dv/(isp*g)))*isp*g/F
 before=solve(node,-dur/2).y[:,-1];before[6]=m
 force=F*node[3:6]/np.linalg.norm(node[3:6]);y=solve(before,dur,force).y[:,-1];return y,dur
y,d=burn(node,dv);print('single',d,'apo km',apo(y),'mass',y[6],'radius at end km',np.linalg.norm(y[:3])/1000)
y1,d1=burn(node,dv/2)
def perigee(t,y):return np.dot(y[:3],y[3:6])
perigee.direction=1;perigee.terminal=True
q=solve(y1,200000,events=perigee);print('coast',q.t[-1],q.status,'first apo',apo(y1));node2=q.y[:,-1];print('next rp',np.linalg.norm(node2[:3])/1000)
y2,d2=burn(node2,dv/2);print('split',d1,d2,'apo km',apo(y2),'mass',y2[6]);print('ideal',apo(node+np.array([0,0,0,0,dv,0,0])))
