import { Game } from "./game/game";
import { bootCore, coreStatus } from "./core/core";
import * as orbitMath from "./core/orbit";

function fail(message: string): void {
  const boot = document.getElementById("boot-status");
  if (boot) boot.textContent = message;
  const inner = document.querySelector(".boot-inner");
  if (inner instanceof HTMLElement) inner.style.filter = "none";
}

async function start(): Promise<void> {
  try {
    await Promise.race([
      document.fonts.ready,
      new Promise((resolve) => window.setTimeout(resolve, 1200)),
    ]);
    // The Rust core is optional: if it will not instantiate the TypeScript
    // implementation keeps flying. `?core=ts` forces the fallback.
    bootCore();
    const game = new Game();
    // Exposed for the visual test harness and for support diagnostics.
    (window as unknown as { odderon?: Game }).odderon = game;
    game.coreStatus = coreStatus();
    // Test seam: the visual harness compares the Rust core against the
    // TypeScript fallback through the same entry points the game uses.
    (window as unknown as { odderonMath?: unknown }).odderonMath = orbitMath;
    requestAnimationFrame(() => {
      const boot = document.getElementById("boot");
      if (!boot) return;
      boot.classList.add("is-done");
      window.setTimeout(() => boot.remove(), 520);
    });
  } catch (error) {
    const message = error instanceof Error ? error.message : String(error);
    fail(`FLIGHT COMPUTER FAILED: ${message.toUpperCase()}`);
    // A visible failure beats an empty black page.
    console.error(error);
  }
}

void start();
