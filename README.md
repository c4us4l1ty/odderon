# Odderon

**[Play and develop Odderon III](odderon-v3/README.md)** — the browser-first orbital engineering game. Fly from the cockpit; use the engineering stations to design the vehicle and predict what it will do. The flight model is the shared v1 Rust workspace, not the v2 TypeScript simulation.

| Tree | Role |
| --- | --- |
| [`odderon-v3`](odderon-v3/) | Current cockpit, local practice game, optional desktop shell and CI-built browser distribution |
| [`odderon-v1`](odderon-v1/) | Scientific engine, engineering instruments, validation data and original planner-based game |
| [`odderon-v2`](odderon-v2/) | Separate earlier cockpit prototype and source of procedural vehicle art |

**V3 has no Cloudflare or other online authority. Server-authoritative Astro-Elo, first-attempt commitments, signed ranked receipts, public replay hosting and ranked leaderboards are postponed.** Local pilot signatures and local replay are not proof of a ranked first attempt. The historical authority implementations remain in v1; they are not services to install for v3.

Steam publication is a future distribution goal, not a requirement for playing in a browser today.
