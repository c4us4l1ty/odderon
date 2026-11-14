# WP 1.3 / 1.4 planning: model contracts and feasibility

This is an implementation-in-progress, not a playable mission loop. The
scenario specifications require additional choices and several numeric gates
must not be presented as universal physical truths.

## Apsides, directions and time quantization

The requested T–N–W frame takes T along velocity, W along orbital angular
momentum, and N = W × T. N points inward at the periapsis of a prograde orbit;
it is generally **not radial** away from an apsis, because velocity has a
radial component. An increase in prograde speed raises the *opposite apsis*
most cleanly when the burn is near an apsis; a general burn changes energy and
both osculating apsides. A normal burn changes orbital angular momentum; torque
is r × F, generally not along the line of nodes. Explanations and diagrams must
not claim these approximations hold everywhere.

The selected oracle convention checks the ideal rocket equation separately
from the explicitly bounded quantization error in the executed command. The
compiler uses integer microseconds and a normalized, quantized quaternion;
it reports both the unquantized rocket-equation mass and commanded mass. For
example, the 18,000 kg tug, 18,000 N, 450 s, 3,120 m/s burn has an ideal
2,236.858757254 s duration; rounding that duration to microseconds changes
consumed mass by roughly 1.04 micrograms. Thus exact rocket-equation mass and
<1e-9 kg after executing an integer-microsecond, fixed-thrust command cannot
both be guaranteed. In the mathematical F → infinity limit a quantized burn
also becomes shorter than one microsecond and is no longer representable.

## Cislunar fixture caveats

For the tug's stated 200 km circular Earth orbit, the ideal, instantaneous,
two-body 3,120 m/s prograde impulse reaches approximately **342,072 km from
Earth's center** (about 335,694 km altitude). Even the lower edge of a
365,000 km *radius* target needs about 3,126.47 m/s under that ideal model.
This does not rule out lunar gravity assists or a different burn budget, but
those require specifying Moon phase, encounter geometry, duration, targeting,
and whether 380,000 km means radius or altitude. A radius crossing alone is
not a lunar encounter.

`tools/astro-oracle/perigee_kicks_scipy.py` is an independent SciPy DOP853
experiment, **not** an operational trajectory: in a point-Earth, constant-
inertial-direction, centered-burn model, one 3,120 m/s burn gives an osculating
apoapsis radius near 41,044 km, versus about 109,315 km after two equal burns
at consecutive predicted periapses. Both consume the same mass. The 2-kick
gain exceeds 40,000 km for these exact assumptions but does not reach the
stated target. Other steering laws and perturbations produce different answers;
there is no proof here that a single burn is universally impossible.

## Attribution is metric- and model-dependent

The selected debrief convention is to label these as model-dependent
sensitivity shares and compute separate shares for position, node and
apoapsis metrics. The proposed 3D position-distance leave-one-out deltas are nonnegative
sensitivity magnitudes, not signed contributions and not a unique additive
breakdown of the actual error. Effects interact and can cancel. Normalizing
these magnitudes produces shares of the sensitivity sum, not exact causal
percentages of apoapsis shortfall or node drift. The requested >85% nodal and
>70% apoapsis criteria need **metric-specific** counterfactual deltas, not the
same position-distance shares. A zero-sensitivity case must report that shares
are undefined instead of inventing a 100% distribution. See the discussion of
interaction effects in [Zhao et al.](https://proceedings.mlr.press/v216/zhao23a/zhao23a.pdf).

## Current implementation progress

The Rust foundation now compiles sorted multi-node ideal conics and quantized
burns, rejects overlapping/out-of-plan commands and planned fuel deficits,
locks an immutable prediction, and performs seven counterfactual propagations.
A first explicit scoring rule exists for the 550 km orbit-rescue objective.
The WebAssembly bridge, interactive handles, accessible lens, mission selector,
debrief and real browser end-to-end mission gate remain to be implemented. The
point-Earth perigee-kick regression is not proof of the complete lunar mission.
