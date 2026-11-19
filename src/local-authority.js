// V3 is an offline game. This adapter has no transport, including on errors.
export const configured = () => false;
const postponed = async () => {
  throw new Error("Ranked Astro-Elo is postponed. This is local practice.");
};
export {
  postponed as commit,
  postponed as submit,
  postponed as registerCallsign,
  postponed as leaderboard,
  postponed as player,
  postponed as publicFlight,
};
