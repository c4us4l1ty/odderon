// Presentation copy only. No masses, engine settings or initial state vectors.
export const catalog = [
  {
    id: 101,
    title: "LEO drag rescue",
    rank: "Cadet",
    brief: "Raise a decaying orbit to a stable 550 km circular orbit.",
    objective:
      "Start at 260 km. Burn prograde to raise apoapsis, coast to the high point, then burn again to circularize within three hours.",
  },
  {
    id: 102,
    title: "Sun-synchronous shift",
    rank: "Flight dynamics officer",
    brief:
      "Circularize at 800 km and tune the orbital plane to follow the Sun.",
    objective:
      "Start in a 250 × 800 km orbit at 96°. Circularize at 800 km and target 98.6° inclination for 4.93° of nodal drift in five days.",
  },
  {
    id: 103,
    title: "Cislunar transfer",
    rank: "Senior flight officer",
    brief:
      "Lift a cryogenic tug from low Earth orbit onto a lunar transfer arc.",
    objective:
      "Use repeated perigee kicks to target a 380,000 km geocentric apoapsis, ±15,000 km. Track propellant loss between burns. The lunar B-plane tool is a planning aid, not a capture guarantee.",
  },
];
export const stations = [
  ["flight", "joystick", "Flight"],
  ["nav", "orbit", "Nav"],
  ["propulsion", "flame", "Engine"],
  ["vehicle", "boxes", "Vehicle"],
  ["academy", "graduation-cap", "Academy"],
  ["obs", "telescope", "Scope"],
];
