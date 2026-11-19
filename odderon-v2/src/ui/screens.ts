/* Menu, contract board, pause, settings and the flight report.
 *
 * These are full-frame surfaces over the live 3D scene. They are built from the
 * same machined-plate vocabulary as the HUD: cut corners, hairline rules,
 * stencilled labels and tabular numbers. */

import { CONTRACTS, Contract, findContract, FlightResult } from "../core/mission";
import { formatDuration } from "../core/math";

export interface Settings {
  grain: boolean;
  orbitLines: boolean;
  invertY: boolean;
  audio: boolean;
  sensitivity: number;
}

const SETTINGS_KEY = "odderon.v2.settings";

const DEFAULT_SETTINGS: Settings = {
  grain: true,
  orbitLines: true,
  invertY: false,
  audio: true,
  sensitivity: 1,
};

const $ = <T extends HTMLElement = HTMLElement>(id: string): T => {
  const el = document.getElementById(id);
  if (!el) throw new Error(`Missing element #${id}`);
  return el as T;
};

const fmt = (n: number): string =>
  n.toLocaleString("en-US", { maximumFractionDigits: 0 }).replace(/,/g, " ");

/** Settings are user-editable storage: never trust the types inside. */
function sanitiseSettings(input: unknown): Settings {
  const out: Settings = { ...DEFAULT_SETTINGS };
  if (!input || typeof input !== "object") return out;
  const raw = input as Record<string, unknown>;
  for (const key of ["grain", "orbitLines", "invertY", "audio"] as const) {
    if (typeof raw[key] === "boolean") out[key] = raw[key] as boolean;
  }
  const scale = raw.sensitivity;
  if (typeof scale === "number" && Number.isFinite(scale)) {
    out.sensitivity = Math.min(3, Math.max(0.2, scale));
  }
  return out;
}

export class Screens {
  onLaunch: (contractId: string) => void = () => {};
  onResume: () => void = () => {};
  onRestart: () => void = () => {};
  onReturnToMenu: () => void = () => {};
  onQuit: () => void = () => {};
  onSettingsChanged: (settings: Settings) => void = () => {};
  /** Which flight core is live; the settings panel reports it. */
  onCoreStatus: () => "idle" | "ready" | "unavailable" = () => "idle";

  settings: Settings = { ...DEFAULT_SETTINGS };
  private selected = 0;
  private menuFocus = 0;
  private pauseFocus = 0;
  /** True while an overlay was opened from the pause menu. */
  private overlayFromPause = false;
  private currentMeter = 0;

  private readonly menu = $("menu");
  private readonly operations = $("operations");
  private readonly pause = $("pause");
  private readonly controls = $("controls");
  private readonly settingsScreen = $("settings");
  private readonly debrief = $("debrief");
  private readonly boot = $("boot");

  constructor() {
    this.loadSettings();
    this.buildContractBoard();
    this.selectContract(0);

    for (const el of document.querySelectorAll<HTMLElement>("[data-action]")) {
      el.addEventListener("click", () => this.menuAction(el.dataset.action ?? ""));
    }
    for (const el of document.querySelectorAll<HTMLElement>("[data-close]")) {
      el.addEventListener("click", () => this.closeOverlay(el.dataset.close ?? ""));
    }
    for (const el of document.querySelectorAll<HTMLElement>("[data-pause]")) {
      el.addEventListener("click", () => this.pauseAction(el.dataset.pause ?? ""));
    }

    $("brief-prev").addEventListener("click", () => this.cycleContract(-1));
    $("brief-next").addEventListener("click", () => this.cycleContract(1));

    const grain = $<HTMLInputElement>("set-grain");
    const orbit = $<HTMLInputElement>("set-orbit-lines");
    const invert = $<HTMLInputElement>("set-invert-y");
    const audio = $<HTMLInputElement>("set-audio");
    const scale = $<HTMLInputElement>("set-scale");
    const scaleVal = $("set-scale-val");

    grain.addEventListener("change", () => this.patchSettings({ grain: grain.checked }));
    orbit.addEventListener("change", () => this.patchSettings({ orbitLines: orbit.checked }));
    invert.addEventListener("change", () => this.patchSettings({ invertY: invert.checked }));
    audio.addEventListener("change", () => this.patchSettings({ audio: audio.checked }));
    scale.addEventListener("input", () => {
      const v = Number(scale.value) / 100;
      scaleVal.textContent = `${Math.round(v * 100)}%`;
      this.patchSettings({ sensitivity: v });
    });

    $("debrief-menu").addEventListener("click", () => this.onReturnToMenu());
    $("debrief-replay").addEventListener("click", () => this.onRestart());
    $("debrief-next").addEventListener("click", () => {
      const next = (this.selected + 1) % CONTRACTS.length;
      this.onLaunch(CONTRACTS[next].id);
    });

    this.applySettings();
  }

  private loadSettings(): void {
    try {
      const raw = localStorage.getItem(SETTINGS_KEY);
      if (raw) {
        const parsed = JSON.parse(raw) as Partial<Settings>;
        this.settings = sanitiseSettings(parsed);
      }
    } catch {
      this.settings = { ...DEFAULT_SETTINGS };
    }
    const grain = $<HTMLInputElement>("set-grain");
    grain.checked = this.settings.grain;
    $<HTMLInputElement>("set-orbit-lines").checked = this.settings.orbitLines;
    $<HTMLInputElement>("set-invert-y").checked = this.settings.invertY;
    $<HTMLInputElement>("set-audio").checked = this.settings.audio;
    const scale = $<HTMLInputElement>("set-scale");
    scale.value = String(Math.round(this.settings.sensitivity * 100));
    $("set-scale-val").textContent = `${Math.round(this.settings.sensitivity * 100)}%`;
  }

  private patchSettings(patch: Partial<Settings>): void {
    this.settings = { ...this.settings, ...patch };
    try {
      localStorage.setItem(SETTINGS_KEY, JSON.stringify(this.settings));
    } catch {
      /* storage is optional */
    }
    this.applySettings();
    this.onSettingsChanged(this.settings);
  }

  private applySettings(): void {
    document.body.classList.toggle("no-grain", !this.settings.grain);
    document.body.classList.toggle("no-orbit", !this.settings.orbitLines);
  }

  get selectedContract(): Contract {
    return CONTRACTS[this.selected] ?? CONTRACTS[0]!;
  }

  private cycleContract(dir: number): void {
    this.selected = (this.selected + dir + CONTRACTS.length) % CONTRACTS.length;
    this.selectContract(this.selected);
  }

  selectContract(index: number): void {
    this.selected = Math.max(0, Math.min(CONTRACTS.length - 1, index));
    const c = this.selectedContract;
    $("brief-name").textContent = c.name;
    $("brief-reward").textContent = fmt(c.reward);
    $("brief-agency").textContent = `${c.agency} · CONTRACT ${c.code.replace("CTR-", "")}`;
    $("brief-copy").textContent = c.detail;
    $("brief-start").textContent = `${fmt(c.start.peAlt / 1000)} × ${fmt(c.start.apAlt / 1000)} KM`;
    $("brief-target").textContent =
      Math.abs(c.target.apAlt - c.target.peAlt) < 2_000
        ? `${fmt(c.target.peAlt / 1000)} KM CIRCULAR`
        : `${fmt(c.target.peAlt / 1000)} × ${fmt(c.target.apAlt / 1000)} KM`;
    $("brief-inc").textContent = `${c.target.inc.toFixed(1)}°`;
    $("brief-window").textContent = formatDuration(c.window);
    $("brief-diff").textContent = c.difficulty;
    $("brief-index").textContent = `${String(this.selected + 1).padStart(2, "0")} / ${String(CONTRACTS.length).padStart(2, "0")}`;
    for (const card of document.querySelectorAll<HTMLElement>(".contract")) {
      card.classList.toggle("is-selected", card.dataset.contract === c.id);
    }
    const canvas = $<HTMLCanvasElement>("brief-canvas");
    this.drawDiagram(canvas, c);
  }

  private drawDiagram(canvas: HTMLCanvasElement, c: Contract): void {
    const ctx = canvas.getContext("2d");
    if (!ctx) return;
    const dpr = Math.min(window.devicePixelRatio || 1, 2);
    const w = canvas.clientWidth || 380;
    const h = canvas.clientHeight || 132;
    canvas.width = Math.floor(w * dpr);
    canvas.height = Math.floor(h * dpr);
    ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
    ctx.clearRect(0, 0, w, h);

    // Grid.
    ctx.strokeStyle = "rgba(141,199,214,0.07)";
    ctx.lineWidth = 1;
    for (let x = 0; x < w; x += 26) {
      ctx.beginPath();
      ctx.moveTo(x, 0);
      ctx.lineTo(x, h);
      ctx.stroke();
    }
    for (let y = 0; y < h; y += 26) {
      ctx.beginPath();
      ctx.moveTo(0, y);
      ctx.lineTo(w, y);
      ctx.stroke();
    }

    const cx = w * 0.5;
    const cy = h * 0.52;
    const maxAlt = Math.max(c.start.apAlt, c.target.apAlt, c.target.peAlt) / 1000;
    const k = (Math.min(w, h) * 0.40) / Math.max(maxAlt, 600);
    const px = (altKm: number): number => 16 + altKm * k;
    const pe = px(c.start.peAlt / 1000);
    const ap = px(c.start.apAlt / 1000);
    const rx = (pe + ap) / 2;
    const ry = Math.sqrt(pe * ap) * 0.62;
    const focus = Math.sqrt(Math.max(0, rx * rx - ry * ry));

    // Earth.
    const earthGrad = ctx.createRadialGradient(cx - 4, cy - 4, 2, cx, cy, 17);
    earthGrad.addColorStop(0, "#2a7fa8");
    earthGrad.addColorStop(0.7, "#0c3550");
    earthGrad.addColorStop(1, "#03101a");
    ctx.fillStyle = earthGrad;
    ctx.beginPath();
    ctx.arc(cx, cy, 16, 0, Math.PI * 2);
    ctx.fill();
    ctx.strokeStyle = "rgba(120,220,244,0.5)";
    ctx.lineWidth = 1;
    ctx.stroke();

    // Current orbit.
    ctx.save();
    ctx.translate(cx - focus * Math.cos(-0.35), cy);
    ctx.rotate(-0.26);
    ctx.beginPath();
    ctx.ellipse(0, 0, rx, ry, 0, 0, Math.PI * 2);
    ctx.strokeStyle = "rgba(99,226,244,0.8)";
    ctx.lineWidth = 1.2;
    ctx.stroke();
    ctx.restore();

    // Target orbit.
    const tr = Math.max(px(c.target.peAlt / 1000), px(c.target.apAlt / 1000));
    ctx.save();
    ctx.setLineDash([3, 4]);
    ctx.beginPath();
    ctx.arc(cx, cy, tr, Math.PI * 1.04, Math.PI * 1.96);
    ctx.strokeStyle = "rgba(94,242,160,0.7)";
    ctx.lineWidth = 1;
    ctx.stroke();
    ctx.restore();

    // Current position marker.
    const nu = (c.start.nu * Math.PI) / 180;
    const a = (pe + ap) / 2;
    const b = Math.sqrt(pe * ap);
    const ex = a * Math.cos(nu);
    const ey = b * Math.sin(nu);
    const rot = -0.26;
    const sx = cx - focus * Math.cos(-0.35) + ex * Math.cos(rot) - ey * Math.sin(rot);
    const sy = cy + ex * Math.sin(rot) + ey * Math.cos(rot);
    ctx.fillStyle = "#ffd38a";
    ctx.beginPath();
    ctx.arc(sx, sy, 2.4, 0, Math.PI * 2);
    ctx.fill();
    ctx.strokeStyle = "rgba(255,211,138,0.5)";
    ctx.beginPath();
    ctx.arc(sx, sy, 5.5, 0, Math.PI * 2);
    ctx.stroke();

    // Corner marks and labels.
    ctx.strokeStyle = "rgba(141,199,214,0.3)";
    ctx.beginPath();
    ctx.moveTo(6, 6);
    ctx.lineTo(20, 6);
    ctx.moveTo(6, 6);
    ctx.lineTo(6, 20);
    ctx.moveTo(w - 6, h - 6);
    ctx.lineTo(w - 20, h - 6);
    ctx.moveTo(w - 6, h - 6);
    ctx.lineTo(w - 6, h - 20);
    ctx.stroke();

    ctx.font = "500 6.5px 'Data', monospace";
    ctx.fillStyle = "rgba(167,189,197,0.6)";
    ctx.fillText("EARTH", 6, h - 8);
    ctx.fillText(`PE ${fmt(c.target.peAlt / 1000)} KM`, w - 96, 16);
    ctx.fillText(`AP ${fmt(c.target.apAlt / 1000)} KM`, w - 96, h - 8);

  }

  private buildContractBoard(): void {
    const board = $("contract-board");
    board.replaceChildren();
    CONTRACTS.forEach((c, i) => {
      const card = document.createElement("button");
      card.type = "button";
      card.className = "contract";
      card.dataset.contract = c.id;
      card.innerHTML = `
        <div class="contract-top"><span class="contract-index">${String(i + 1).padStart(2, "0")}</span><span>${c.code}</span><span>${c.difficulty}</span></div>
        <h3>${c.name}</h3>
        <p class="agency">${c.agency}</p>
        <p>${c.brief}</p>
        <div class="contract-orbit">
          <div><span>CURRENT</span><b>${fmt(c.start.peAlt / 1000)} × ${fmt(c.start.apAlt / 1000)} km</b></div>
          <div><span>TARGET</span><b>${fmt(c.target.peAlt / 1000)} × ${fmt(c.target.apAlt / 1000)} km</b></div>
          <div><span>INCLINATION</span><b>${c.target.inc.toFixed(1)}°</b></div>
          <div><span>WINDOW</span><b>${formatDuration(c.window)}</b></div>
        </div>
        <div class="contract-reward"><span>RATING · ${c.difficulty}</span><b>◈ ${fmt(c.reward)}</b></div>
      `;
      card.addEventListener("click", () => {
        this.selectContract(i);
        this.closeOverlay("operations");
        this.showMenu();
      });
      board.append(card);
    });
  }

  private menuAction(action: string): void {
    switch (action) {
      case "launch":
        this.onLaunch(this.selectedContract.id);
        break;
      case "operations":
        this.closeOtherOverlays("operations");
        this.operations.classList.remove("is-hidden");
        break;
      case "settings":
        this.closeOtherOverlays("settings");
        this.refreshSystemInfo();
        this.settingsScreen.classList.remove("is-hidden");
        break;
      case "quit":
        this.onQuit();
        break;
      default:
        break;
    }
  }

  /** Fill the read-only system column of the settings panel. */
  private refreshSystemInfo(): void {
    const canvas = document.querySelector<HTMLCanvasElement>("#viewport");
    const gl = canvas?.getContext("webgl2");
    let renderer = "WEBGL2";
    if (gl) {
      const dbg = gl.getExtension("WEBGL_debug_renderer_info");
      const raw = (dbg ? String(gl.getParameter(dbg.UNMASKED_RENDERER_WEBGL)) : "software").toUpperCase();
      if (raw.includes("SWIFTSHADER")) renderer = "SWIFTSHADER · SOFTWARE";
      else if (raw.includes("ANGLE")) renderer = "ANGLE · HARDWARE";
      else renderer = raw.replace(/^GOOGLE, /, "").slice(0, 26);
    }
    const r = $("set-renderer");
    if (r) r.textContent = renderer || "—";
    const core = $("set-core");
    if (core) {
      const status = this.onCoreStatus();
      core.textContent =
        status === "ready" ? "RUST · WASM · 24 KB" : status === "unavailable" ? "TYPESCRIPT · FALLBACK" : "BOOTING";
    }
    const v = $("set-viewport");
    if (v) v.textContent = `${window.innerWidth} × ${window.innerHeight} · ${Math.round(window.devicePixelRatio * 100)}%`;
  }

  private pauseAction(action: string): void {
    switch (action) {
      case "resume":
        this.hideOverlays();
        this.onResume();
        break;
      case "restart":
        this.hideOverlays();
        this.onRestart();
        break;
      case "controls":
        this.overlayFromPause = true;
        this.hidePause();
        this.closeOtherOverlays("controls");
        this.controls.classList.remove("is-hidden");
        break;
      case "settings":
        this.overlayFromPause = true;
        this.hidePause();
        this.closeOtherOverlays("settings");
        this.refreshSystemInfo();
        this.settingsScreen.classList.remove("is-hidden");
        break;
      case "menu":
        this.hideOverlays();
        this.onReturnToMenu();
        break;
      default:
        break;
    }
  }

  closeOverlay(id: string): void {
    this.hideOne(id);
    this.restorePauseIfNested();
  }

  private hideOne(id: string): void {
    const map: Record<string, HTMLElement> = {
      operations: this.operations,
      controls: this.controls,
      settings: this.settingsScreen,
    };
    map[id]?.classList.add("is-hidden");
  }

  /** Closing an overlay that was opened from the pause menu returns there. */
  private restorePauseIfNested(): void {
    if (!this.overlayFromPause) return;
    if (this.overlaysOpen()) return;
    this.overlayFromPause = false;
    this.pause.classList.remove("is-hidden");
  }

  private closeOtherOverlays(except: string): void {
    // No pause restore here: this runs while *opening* an overlay, when the
    // pause card is supposed to stay hidden until the overlay is closed.
    for (const id of ["operations", "controls", "settings"]) {
      if (id !== except) this.hideOne(id);
    }
  }

  hideOverlays(): void {
    for (const el of [this.operations, this.controls, this.settingsScreen, this.pause, this.debrief]) {
      el.classList.add("is-hidden");
    }
    this.overlayFromPause = false;
  }

  overlaysOpen(): boolean {
    return !this.operations.classList.contains("is-hidden") ||
      !this.controls.classList.contains("is-hidden") ||
      !this.settingsScreen.classList.contains("is-hidden");
  }

  showMenu(): void {
    this.hideOverlays();
    this.menu.classList.remove("is-hidden");
  }

  hideMenu(): void {
    this.menu.classList.add("is-hidden");
  }

  showFlight(): void {
    this.hideMenu();
    this.hideOverlays();
  }

  showPause(met: number): void {
    this.currentMeter = met;
    $("pause-met").textContent = `T+ ${formatDuration(met)}`;
    this.pause.classList.remove("is-hidden");
  }

  hidePause(): void {
    this.pause.classList.add("is-hidden");
  }

  debriefOpen(): boolean {
    return !this.debrief.classList.contains("is-hidden");
  }

  showDebrief(result: FlightResult): void {
    const c = result.contract;
    // Follow the contract that was actually flown, not whatever the board had
    // highlighted: "next operation" must be the one after *this* flight.
    const flown = CONTRACTS.findIndex((entry) => entry.id === c.id);
    if (flown >= 0) this.selected = flown;
    $("debrief-name").textContent = c.name.toUpperCase();
    $("debrief-code").textContent = `CONTRACT ${c.code} · ${c.agency}`;
    $("debrief-grade").textContent = result.grade;
    $("debrief-score").textContent = String(result.score).padStart(4, "0");
    $("debrief-ap").textContent = `${fmt(c.target.apAlt / 1000 + result.apError / 1000)} km`;
    $("debrief-pe").textContent = `${fmt(c.target.peAlt / 1000 + result.peError / 1000)} km`;
    $("debrief-ap-err").textContent = `${result.apError >= 0 ? "+" : "−"}${(Math.abs(result.apError) / 1000).toFixed(1)} km`;
    $("debrief-pe-err").textContent = `${result.peError >= 0 ? "+" : "−"}${(Math.abs(result.peError) / 1000).toFixed(1)} km`;
    $("debrief-fuel").textContent = `${fmt(result.fuelUsed)} kg`;
    $("debrief-time").textContent = formatDuration(result.missionTime);

    const status = $("report-status");
    status.classList.toggle("failed", !result.passed);
    const b = status.querySelector("b");
    if (b) b.textContent = result.passed ? "SECURED" : result.accuracy > 0.4 ? "PARTIAL" : "ABORTED";

    const grade = $("debrief-grade");
    grade.style.color = result.passed ? "var(--green)" : "var(--red)";
    grade.style.textShadow = result.passed
      ? "0 0 40px rgba(94,242,160,0.35)"
      : "0 0 40px rgba(255,91,69,0.3)";

    const stamp = $("debrief-stamp");
    stamp.textContent = result.passed ? "FILED" : "REJECTED";
    stamp.classList.toggle("failed", !result.passed);

    const notes: string[] = [];
    notes.push(
      result.passed
        ? "The target orbit is inside contract tolerance. Apsides and inclination were confirmed from the flown state vector."
        : "The target orbit was not reached inside tolerance. The state vector was recorded at flight termination.",
    );
    const worst = Math.max(Math.abs(result.peError), Math.abs(result.apError)) / 1000;
    notes.push(`Largest apsis miss: ${worst.toFixed(1)} km. Propellant consumed: ${fmt(result.fuelUsed)} kg of 2 500 kg.`);
    notes.push(
      result.score >= 850
        ? "Burn economy was efficient. The flight office has logged the trajectory for the ranked board."
        : "The flight office recommends a smaller initial burn and a trim burn at the new apsis.",
    );
    $("debrief-notes").textContent = notes.join(" ");
    $("debrief-next").style.display = CONTRACTS.length > 1 ? "" : "none";
    $("debrief-next").textContent = nextLabel(this.selected);
    this.hideOverlays();
    this.debrief.classList.remove("is-hidden");
  }

  hideDebrief(): void {
    this.debrief.classList.add("is-hidden");
  }

  hideBoot(): void {
    this.boot.classList.add("is-done");
    window.setTimeout(() => this.boot.remove(), 500);
  }

  /** Menu keyboard focus. Returns true when the key was consumed. */
  key(event: KeyboardEvent): boolean {
    if (!this.pause.classList.contains("is-hidden")) {
      if (event.key === "Escape") {
        this.hidePause();
        this.onResume();
        return true;
      }
      return false;
    }
    if (this.debriefOpen()) return false;
    if (this.overlaysOpen()) {
      if (event.key === "Escape") {
        this.closeOtherOverlays("");
        this.restorePauseIfNested();
        return true;
      }
      return false;
    }
    if (this.menu.classList.contains("is-hidden")) return false;

    const items = Array.from(this.menu.querySelectorAll<HTMLButtonElement>(".menu-list .menu-item"));
    if (event.key === "ArrowDown") {
      this.menuFocus = (this.menuFocus + 1) % items.length;
      items.forEach((el, i) => el.classList.toggle("is-selected", i === this.menuFocus));
      return true;
    }
    if (event.key === "ArrowUp") {
      this.menuFocus = (this.menuFocus - 1 + items.length) % items.length;
      items.forEach((el, i) => el.classList.toggle("is-selected", i === this.menuFocus));
      return true;
    }
    if (event.key === "Enter") {
      items[this.menuFocus]?.click();
      return true;
    }
    if (event.key === "ArrowRight" || event.key === "ArrowLeft") {
      this.cycleContract(event.key === "ArrowRight" ? 1 : -1);
      return true;
    }
    void this.currentMeter;
    return false;
  }

  setPauseFocus(index: number): void {
    this.pauseFocus = index;
    void this.pauseFocus;
  }
}

function nextLabel(selected: number): string {
  const next = CONTRACTS[(selected + 1) % CONTRACTS.length];
  return `NEXT OPERATION · ${findContract(next?.id ?? "").name.toUpperCase()} ▸`;
}
