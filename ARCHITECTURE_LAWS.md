The 5 Global "Anti-Hallucination Laws"

Before starting any Block, your AI agent must obey these five invariant laws across the entire project:

1. The Dimensional Unit Law: Every variable and data contract must strictly use SI Base Units internally: meters (mm), kilograms (kgkg), seconds (ss), Kelvin (KK), Pascals (PaPa), Watts (WW), radians (radrad), and Joules (JJ). Display conversions (e.g., kmkm, daysdays, light-yearslight-years, RPMRPM) are permitted only in the UI formatting layer.
  
2. The Zero-Polling Idle Law: requestAnimationFrame must never run continuously by default. Rendering and physics computation are strictly Event-Driven and Dirty-Flagged. When the camera is stationary and time-warp is paused, CPU usage must drop to 0.0% and frame redraws must drop to 0 FPS.
  
3. The No-Euler Integration Law: First-order Euler integration (xn+1=xn+v⋅Δtxn+1​=xn​+v⋅Δt) and fixed-step RK4 are strictly banned for orbital mechanics because they artificially gain or lose orbital energy. All numerical trajectories must use either Symplectic Integrators (Velocity Verlet / Yoshida) or Adaptive Dormand-Prince (RK45 / RK89) cached into Hermite Cubic Splines.

4. The No-Magic-Lookup Law: No engine, habitat, or subsystem may return hardcoded performance scores (e.g., if (engine == "ion") return 3000;). Every output metric must be algebraically derived from fundamental physical inputs (temperature, molar mass, pressure, area, efficiency).
   
5. The NASA Oracle Gate: No Work Package is considered complete until it passes its numerical Ground-Truth Oracle Test against real NASA/Vallado textbook reference values.
