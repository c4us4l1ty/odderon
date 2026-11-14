# Numerical propagation work in progress

`odd-astro` now contains an adaptive forward DOPRI5(4)7 integrator, a
structure-of-arrays cubic Hermite trajectory cache, and independent force
building blocks. The integrator accepts a caller's SI, inertial-frame force
callback and explicit sorted event times. Every accepted step is stored; an
end-stage at a supplied event uses the left-hand limit, and the next step uses
the right-hand force. This is not yet a complete mission propagator: callers
must assemble gravity, ephemerides and forces themselves, supply burn events,
and enforce constraints such as dry mass and impact. It has no state-triggered
impact or SOI detection, backwards integration, worker bridge, or UI display.

J2 accepts an arbitrary fixed spin axis. Third-body gravity uses the Battin
form. The drag helper assumes an Earth reference atmosphere and a co-rotating
oblate body: the seven lower USSA76 layers end near 86 km, and above that it
uses a fixed exponential reference table, not full USSA76 upper-atmosphere
physics. Real thermospheric density varies strongly with solar activity. Solar
radiation pressure is a cannonball model with an approximate one-occulter
angular-disk penumbra; combined Earth/Moon shadow overlap is not modeled.
The burn helper supplies constant inertial force and constant Isp, and does not
implement guidance or dry-mass cutoff. These building blocks are not yet
validated to the full requested Level-3 oracle suite or native/Wasm bit parity;
do not use them for operational flight predictions.

The single-pass renderer and ephemeris blend have their own caveats in
[observatory.md](observatory.md) and [ephemeris.md](ephemeris.md). SOI-clipped
hyperbolic trajectories and physical spacecraft planet-shine are not yet
implemented.
