/* Binds the flight state to the instrument DOM. All text updates are cached so
 * a frame never touches a node whose value did not change. */

import { Quaternion, Vector3 } from "three";
import { EARTH_RADIUS, clamp, deg, formatDuration } from "../core/math";
import { Elements } from "../core/orbit";
import { Contract, Evaluation } from "../core/mission";
import { AttitudeInfo, Navball } from "./navball";

export type SasMode = "off" | "stability" | "prograde" | "retrograde" | "radial" | "normal" | "node";
export type CameraMode = "flight" | "chase" | "map";

export interface HudData {
  contract: Contract;
  elements: Elements;
  target: Elements;
  evaluation: Evaluation;
  met: number;
  warp: number;
  fps: number;
  throttle: number;
  armed: boolean;
  sas: SasMode;
  rcs: boolean;
  propellant: number;
  fuelFraction: number;
  mass: number;
  thrust: number;
  deltaV: number;
  attitude: AttitudeInfo;
  orientation: Quaternion;
  position: Vector3;
  velocity: Vector3;
  node: { active: boolean; dv: number; time: number; burnTime: number; planned: Elements | null } | null;
  autopilot: string;
  complete: boolean;
  closed: boolean;
  paused: boolean;
}

type AlertKind = "warn" | "danger" | "good";

const $ = <T extends HTMLElement = HTMLElement>(id: string): T => {
  const el = document.getElementById(id);
  if (!el) throw new Error(`Missing element #${id}`);
  return el as T;
};

const fmt = (n: number, digits = 0): string => {
  if (!Number.isFinite(n)) return "—";
  const s = n.toLocaleString("en-US", { minimumFractionDigits: digits, maximumFractionDigits: digits });
  return s.replace(/,/g, " ");
};

const signed = (n: number, digits = 1): string => {
  if (!Number.isFinite(n)) return "—";
  const sign = n < 0 ? "−" : "+";
  return `${sign}${fmt(Math.abs(n), digits)}`;
};

export class Hud {
  readonly navball: Navball;
  private readonly el: Record<string, HTMLElement> = {};
  private lastValues = new Map<string, string>();
  private lastNav = 0;
  private readonly alerts = new Map<string, HTMLDivElement>();
  private readonly root = $("hud");

  onThrottle: (v: number) => void = () => {};
  onEngineToggle: () => void = () => {};
  onSas: (mode: SasMode) => void = () => {};
  onRcs: () => void = () => {};
  onWarp: (n: number) => void = () => {};
  onCamera: (mode: CameraMode) => void = () => {};
  onAddNode: () => void = () => {};
  onClearNode: () => void = () => {};
  onNodeDv: (dv: number) => void = () => {};
  onExecute: () => void = () => {};
  onEndFlight: () => void = () => {};

  constructor() {
    const ids = [
      "hud-code", "hud-met", "hud-clock", "hud-warp", "hud-warp-chip", "hud-top-pe", "hud-top-ap",
      "hud-top-inc", "obj-code", "obj-title", "obj-state", "obj-hint", "obj-progress",
      "objective", "obj-lamp", "nav-pitch", "nav-heading", "nav-roll", "plant-prop", "plant-dv",
      "plant-mass", "plant-thrust", "plant-bar", "engine-state", "throttle-num", "throttle-fill",
      "throttle-handle", "rcs-rate", "rcs-mode", "rcs-pad", "orb-ap", "orb-pe", "orb-alt", "orb-vel",
      "orb-ecc", "orb-inc", "orb-period", "orb-sma", "ap-pe-alt", "ap-pe-target", "tgt-ap", "tgt-pe",
      "tgt-ap-err", "tgt-pe-err", "tgt-tol", "tgt-acquire", "node-empty", "node-active", "node-time",
      "node-dv", "node-slider", "node-ap", "node-pe", "node-burn", "btn-execute", "btn-end-flight",
      "thr-mini-num", "thr-mini-fill", "btn-engine", "btn-rcs", "alerts", "sas-modes", "warp-modes",
      "screen-markers", "hud-phase", "panel-node",
    ];
    for (const id of ids) {
      const node = document.getElementById(id);
      if (node) this.el[id] = node;
    }

    this.navball = new Navball($<HTMLCanvasElement>("navball"));
    this.resize();

    // Controls.
    $("btn-engine").addEventListener("click", () => this.onEngineToggle());
    $("btn-rcs").addEventListener("click", () => this.onRcs());
    $("btn-add-node").addEventListener("click", () => this.onAddNode());
    $("btn-execute").addEventListener("click", () => this.onExecute());
    $("btn-end-flight").addEventListener("click", () => this.onEndFlight());

    for (const btn of document.querySelectorAll<HTMLButtonElement>("[data-sas]")) {
      btn.addEventListener("click", () => this.onSas(btn.dataset.sas as SasMode));
    }
    for (const btn of document.querySelectorAll<HTMLButtonElement>("[data-warp]")) {
      btn.addEventListener("click", () => this.onWarp(Number(btn.dataset.warp)));
    }
    for (const btn of document.querySelectorAll<HTMLButtonElement>("[data-cam]")) {
      btn.addEventListener("click", () => this.onCamera(btn.dataset.cam as CameraMode));
    }
    for (const btn of document.querySelectorAll<HTMLButtonElement>("[data-dv]")) {
      btn.addEventListener("click", () => this.onNodeDv(Number(btn.dataset.dv)));
    }

    const slider = $<HTMLInputElement>("node-slider");
    slider.addEventListener("input", () => this.onNodeDv(Number(slider.value) - this.currentNodeDv));
    this.el["node-slider"] = slider;

    // Throttle: drag the rail, or use the inline readout buttons.
    const rail = $("throttle-rail");
    const setFromEvent = (ev: PointerEvent): void => {
      const rect = rail.getBoundingClientRect();
      const v = clamp((ev.clientX - rect.left) / rect.width, 0, 1);
      this.onThrottle(v);
    };
    let dragging = false;
    rail.addEventListener("pointerdown", (ev) => {
      dragging = true;
      rail.setPointerCapture(ev.pointerId);
      setFromEvent(ev);
    });
    rail.addEventListener("pointermove", (ev) => {
      if (dragging) setFromEvent(ev);
    });
    rail.addEventListener("pointerup", (ev) => {
      dragging = false;
      rail.releasePointerCapture(ev.pointerId);
    });

    window.addEventListener("resize", () => this.resize());
  }

  private currentNodeDv = 0;

  resize(): void {
    const wrap = document.querySelector<HTMLElement>(".navball-wrap");
    const size = wrap ? wrap.clientWidth - 18 : 220;
    this.navball.resize(Math.min(size, window.innerHeight * 0.24));
  }

  private set(id: string, value: string): void {
    if (this.lastValues.get(id) === value) return;
    this.lastValues.set(id, value);
    const node = this.el[id];
    if (node) node.textContent = value;
  }

  setVisible(visible: boolean): void {
    this.root.classList.toggle("is-hidden", !visible);
    this.root.setAttribute("aria-hidden", String(!visible));
  }

  update(data: HudData): void {
    const e = data.elements;
    const alt = data.position.length() - EARTH_RADIUS;
    const speed = data.velocity.length();
    const apAlt = Number.isFinite(e.ap) ? e.ap - EARTH_RADIUS : Infinity;
    const peAlt = e.pe - EARTH_RADIUS;

    this.set("hud-code", data.contract.code);
    this.set("hud-met", formatDuration(data.met));
    this.set("hud-clock", `T+ ${formatDuration(data.met)}`);
    this.set("hud-warp", `${data.warp}×`);
    this.set("hud-top-pe", Number.isFinite(peAlt) ? `${fmt(peAlt / 1000)} KM` : "—");
    this.set("hud-top-ap", Number.isFinite(apAlt) ? `${fmt(apAlt / 1000)} KM` : "OPEN");
    this.set("hud-top-inc", `${fmt(deg(e.i), 1)}°`);

    const warpChip = this.el["hud-warp-chip"];
    if (warpChip) warpChip.classList.toggle("is-active", data.warp > 1);

    // Objective.
    this.set("obj-code", data.contract.code);
    this.set("obj-title", objectiveTitle(data.contract));
    this.set("obj-state", data.closed ? "WINDOW CLOSED" : data.complete ? "SECURED" : data.evaluation.inTolerance ? "ON TARGET" : "ACTIVE");
    this.set("obj-hint", objectiveHint(data.contract, data.evaluation));
    const progress = clamp(data.complete ? 1 : data.evaluation.progress, 0, 1);
    const bar = this.el["obj-progress"];
    if (bar) bar.style.width = `${(progress * 100).toFixed(1)}%`;
    this.el["objective"]?.classList.toggle("is-complete", data.complete);

    // Attitude.
    this.set("nav-pitch", `${signed(deg(data.attitude.pitch), 1)}°`);
    const hdg = ((deg(data.attitude.heading) + 360) % 360);
    this.set("nav-heading", `${fmt(hdg, 0).padStart(3, "0")}°`);
    this.set("nav-roll", `${signed(deg(data.attitude.roll), 1)}°`);

    // Propulsion.
    this.set("plant-prop", fmt(data.propellant, 0));
    this.set("plant-dv", fmt(data.deltaV, 0));
    this.set("plant-mass", fmt(data.mass, 0));
    this.set("plant-thrust", fmt(data.thrust / 1000, 1));
    const plantBar = this.el["plant-bar"];
    if (plantBar) plantBar.style.setProperty("--fill", `${(data.fuelFraction * 100).toFixed(1)}%`);
    const state = this.el["engine-state"];
    if (state) {
      state.classList.toggle("is-armed", data.armed && data.throttle <= 0.01);
      state.classList.toggle("is-burning", data.armed && data.throttle > 0.01);
      const b = state.querySelector("b");
      if (b) b.textContent = !data.armed ? "SAFE" : data.throttle > 0.01 ? "BURN" : "ARMED";
    }
    const pct = Math.round(data.throttle * 100);
    this.set("throttle-num", `${String(pct).padStart(3, "0")}%`);
    this.set("thr-mini-num", `${pct}%`);
    const fill = this.el["throttle-fill"];
    if (fill) fill.style.width = `${pct}%`;
    const handle = this.el["throttle-handle"];
    if (handle) handle.style.left = `${pct}%`;
    const mini = this.el["thr-mini-fill"];
    if (mini) mini.style.width = `${pct}%`;
    const engineKey = this.el["btn-engine"];
    if (engineKey) {
      engineKey.classList.toggle("is-armed", data.armed);
      const b = engineKey.querySelector("b");
      if (b) b.textContent = data.armed ? "ARMED" : "SAFE";
    }

    // RCS panel.
    this.set("rcs-rate", `${fmt(deg(0.0), 2)}°/s`);
    this.set("rcs-mode", data.rcs ? "RCS" : "WHEEL");
    const pad = this.el["rcs-pad"];
    if (pad) {
      for (const cell of pad.querySelectorAll<HTMLElement>("i")) {
        const dir = cell.dataset.dir ?? "";
        const lit =
          (dir === "pitch-fwd" && data.attitude.pitch < -0.02) ||
          (dir === "pitch-aft" && data.attitude.pitch > 0.02) ||
          (dir === "yaw-left" && Math.abs(data.attitude.heading) > 0.02 && data.sas === "node") ||
          (dir === "yaw-right" && Math.abs(data.attitude.heading) < -0.02) ||
          (dir === "roll-left" && data.attitude.roll < -0.02) ||
          (dir === "roll-right" && data.attitude.roll > 0.02);
        cell.classList.toggle("is-lit", Boolean(lit) && data.rcs);
      }
    }

    // Orbit readout.
    this.set("orb-ap", Number.isFinite(apAlt) ? fmt(apAlt / 1000) : "OPEN");
    this.set("orb-pe", fmt(peAlt / 1000));
    this.set("orb-alt", `${fmt(alt / 1000, 0)} km`);
    this.set("orb-vel", `${fmt(speed, 0)} m/s`);
    this.set("orb-ecc", Number.isFinite(e.e) ? fmt(e.e, 4) : "—");
    this.set("orb-inc", `${fmt(deg(e.i), 2)}°`);
    this.set("orb-period", Number.isFinite(e.period) ? formatDuration(e.period) : "OPEN");
    this.set("orb-sma", Number.isFinite(e.a) ? `${fmt(e.a / 1000)} km` : "—");

    // Apsis / target rail.
    const altBar = this.el["ap-pe-alt"];
    if (altBar) {
      const a = Number.isFinite(apAlt) ? apAlt : 2_000_000;
      const t = clamp(alt / Math.max(a, EARTH_RADIUS * 0.05), 0, 1);
      altBar.style.left = `${(4 + t * 92).toFixed(1)}%`;
    }
    const targetBar = this.el["ap-pe-target"];
    if (targetBar) {
      const tol = data.contract.target.tolerance;
      const tPe = data.contract.target.peAlt;
      const tAp = data.contract.target.apAlt;
      const lo = clamp(((tPe - tol) / 2_000_000) * 92 + 4, 0, 100);
      const hi = clamp(((tAp + tol) / 2_000_000) * 92 + 4, 0, 100);
      targetBar.style.left = `${lo}%`;
      targetBar.style.width = `${Math.max(1, hi - lo)}%`;
    }

    // Target panel.
    this.set("tgt-ap", `${fmt(data.contract.target.apAlt / 1000)} km`);
    this.set("tgt-pe", `${fmt(data.contract.target.peAlt / 1000)} km`);
    this.set("tgt-tol", fmt(data.contract.target.tolerance / 1000));
    const apErr = data.evaluation.apError;
    const peErr = data.evaluation.peError;
    this.set("tgt-ap-err", `${signed(apErr / 1000, 1)} km`);
    this.set("tgt-pe-err", `${signed(peErr / 1000, 1)} km`);
    for (const [id, err] of [["tgt-ap-err", apErr], ["tgt-pe-err", peErr]] as const) {
      const node = this.el[id];
      if (!node) continue;
      const good = Math.abs(err) <= data.contract.target.tolerance;
      node.classList.toggle("good", good);
      node.classList.toggle("warn", !good);
    }
    const acq = this.el["tgt-acquire"];
    if (acq) acq.style.width = `${(data.evaluation.accuracy * 100).toFixed(1)}%`;

    // Node panel.
    const nodeActive = Boolean(data.node?.active);
    this.el["node-empty"]?.classList.toggle("is-hidden", nodeActive);
    this.el["node-active"]?.classList.toggle("is-hidden", !nodeActive);
    this.el["panel-node"]?.classList.toggle("has-node", nodeActive);
    if (data.node?.active) {
      this.set("node-time", `T+ ${formatDuration(data.node.time)}`);
      this.set("node-dv", fmt(data.node.dv, 0));
      this.set("node-burn", Number.isFinite(data.node.burnTime) ? formatDuration(data.node.burnTime) : "—");
      const planned = data.node.planned;
      this.set("node-ap", planned && Number.isFinite(planned.ap) ? `${fmt((planned.ap - EARTH_RADIUS) / 1000)} km` : "OPEN");
      this.set("node-pe", planned ? `${fmt((planned.pe - EARTH_RADIUS) / 1000)} km` : "—");
      const slider = this.el["node-slider"] as HTMLInputElement | undefined;
      if (slider && document.activeElement !== slider) {
        const max = Number(slider.max);
        if (data.node.dv > max) slider.max = String(Math.ceil(data.node.dv / 500) * 500);
        slider.value = String(Math.round(data.node.dv));
      }
      this.currentNodeDv = Math.round(data.node.dv);
      const exec = this.el["btn-execute"];
      if (exec) {
        exec.textContent = data.autopilot === "idle" ? "ALIGN & EXECUTE" : data.autopilot.toUpperCase();
        exec.classList.toggle("is-on", data.autopilot !== "idle");
      }
    }

    // Engine fires? Stop autopilot spam.
    this.set("hud-phase", data.autopilot !== "idle" ? data.autopilot.toUpperCase() : "MISSION TIME");

    // Buttons.
    for (const btn of document.querySelectorAll<HTMLButtonElement>("[data-sas]")) {
      btn.classList.toggle("is-on", btn.dataset.sas === data.sas);
    }
    for (const btn of document.querySelectorAll<HTMLButtonElement>("[data-warp]")) {
      btn.classList.toggle("is-on", Number(btn.dataset.warp) === data.warp);
    }
    this.el["btn-rcs"]?.classList.toggle("is-on", data.rcs);

    // Navball at ~30 Hz is plenty and keeps the canvas off the critical path.
    const now = performance.now();
    if (now - this.lastNav > 33) {
      this.lastNav = now;
      this.navball.render(data.orientation, data.position, data.velocity);
    }

    this.updateAlerts(data);
  }

  private updateAlerts(data: HudData): void {
    const wanted = new Map<string, { text: string; kind: AlertKind }>();
    if (!data.paused) {
      if (data.fuelFraction <= 0) wanted.set("dry", { text: "PROPELLANT DEPLETED · ENGINE FLAMEOUT", kind: "danger" });
      else if (data.fuelFraction < 0.18) wanted.set("low", { text: `PROPELLANT LOW · ${(data.fuelFraction * 100).toFixed(0)}%`, kind: "warn" });
      if (data.complete) wanted.set("acquired", { text: "TARGET ORBIT ACQUIRED · HOLDING", kind: "good" });
      if (data.autopilot !== "idle") wanted.set("auto", { text: `AUTOPILOT · ${data.autopilot.toUpperCase()}`, kind: "warn" });
      if (data.warp > 1) wanted.set("warp", { text: `TIME WARP ${data.warp}×`, kind: "warn" });
      if (data.closed) wanted.set("closed", { text: "CONTRACT WINDOW CLOSED", kind: "danger" });
    }
    const host = this.el["alerts"];
    if (!host) return;

    for (const [key, node] of this.alerts) {
      if (!wanted.has(key)) {
        node.remove();
        this.alerts.delete(key);
      }
    }
    for (const [key, spec] of wanted) {
      let node = this.alerts.get(key);
      if (!node) {
        node = document.createElement("div");
        node.className = `alert ${spec.kind}`;
        node.textContent = spec.text;
        host.appendChild(node);
        this.alerts.set(key, node);
      } else if (node.textContent !== spec.text) {
        node.textContent = spec.text;
      }
      node.className = `alert ${spec.kind}`;
    }
  }

  flash(text: string, kind: AlertKind = "good", ms = 2600): void {
    const host = this.el["alerts"];
    if (!host) return;
    const node = document.createElement("div");
    node.className = `alert ${kind}`;
    node.textContent = text;
    host.appendChild(node);
    window.setTimeout(() => node.remove(), ms);
  }

  setSasMode(mode: SasMode): void {
    for (const btn of document.querySelectorAll<HTMLButtonElement>("[data-sas]")) {
      btn.classList.toggle("is-on", btn.dataset.sas === mode);
    }
  }

  setCamera(mode: CameraMode): void {
    for (const btn of document.querySelectorAll<HTMLButtonElement>("[data-cam]")) {
      btn.classList.toggle("is-on", btn.dataset.cam === mode);
    }
  }

  setWarp(warp: number): void {
    for (const btn of document.querySelectorAll<HTMLButtonElement>("[data-warp]")) {
      btn.classList.toggle("is-on", Number(btn.dataset.warp) === warp);
    }
  }

  setMet(met: number): void {
    this.set("pause-met", `T+ ${formatDuration(met)}`);
  }
}

function objectiveTitle(contract: Contract): string {
  const t = contract.target;
  if (contract.kind === "deorbit") {
    return `Disposal periapsis at ${fmt((t.peAlt ?? 0) / 1000)} km`;
  }
  if (Math.abs(t.apAlt - t.peAlt) < 2_000) {
    return `Circularise at ${fmt(t.peAlt / 1000)} km`;
  }
  return `Reach ${fmt(t.peAlt / 1000)} × ${fmt(t.apAlt / 1000)} km`;
}

function objectiveHint(contract: Contract, ev: Evaluation): string {
  const tol = contract.target.tolerance;
  if (ev.inTolerance) return "MAINTAIN ORBIT";
  if (ev.apError > tol) return "LOWER APOAPSIS";
  if (ev.peError < -tol) return "RAISE PERIAPSIS";
  if (ev.peError > tol) return "LOWER PERIAPSIS";
  if (ev.apError < -tol) return "RAISE APOAPSIS";
  return "CONVERGE ORBIT";
}
