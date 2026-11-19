/* Odderon flight loop.
 *
 * One state machine drives the menu, the flight and the report. Physics is
 * advanced on a fixed substep while the engine is lit and analytically while
 * coasting, so a 100× warp costs the same as real time and never accumulates
 * Kepler drift. */

import { Quaternion, Vector3 } from "three";
import {
  CONTRACTS,
  Contract,
  Evaluation,
  evaluate,
  findContract,
  initialFlightState,
  scoreFlight,
  targetElements,
} from "../core/mission";
import { EARTH_MU, EARTH_RADIUS, G0, clamp } from "../core/math";
import {
  Elements,
  basisQuaternion,
  elementsFromState,
  localFrameQuaternion,
  propagateKepler,
  stepRK4,
  timeToApsis,
} from "../core/orbit";
import { KESTREL, Vessel, VesselInput } from "../core/vessel";
import { CameraMode, FlightView } from "../render/scene";
import { Audio } from "../ui/audio";
import { Hud, HudData, SasMode } from "../ui/hud";
import { attitudeInfo } from "../ui/navball";
import { Screens } from "../ui/screens";

type Mode = "menu" | "flight" | "debrief";
type Autopilot = "idle" | "waiting" | "aligning" | "burning" | "complete";

interface PlanNode {
  dv: number;
  time: number;
}

export class Game {
  private readonly view: FlightView;
  private readonly hud: Hud;
  private readonly screens: Screens;
  private readonly audio = new Audio();

  /** Which implementation the flight core is running: set by main.ts. */
  coreStatus: "idle" | "ready" | "unavailable" = "idle";

  private mode: Mode = "menu";
  private paused = false;
  private contract: Contract = CONTRACTS[0]!;
  private vessel!: Vessel;
  private target!: Elements;
  private evaluation!: Evaluation;

  private met = 0;
  private warp = 1;
  private throttle = 0;
  private engineArmed = false;
  private sas: SasMode = "prograde";
  private rcs = false;

  private node: PlanNode | null = null;
  private planned: Elements | null = null;
  private nodePosition: Vector3 | null = null;
  private autopilot: Autopilot = "idle";
  private burnDone = 0;
  private burnSign = 1;
  private burnDuration = 0;
  private burnNodeTime = 0;
  private burnObjective: "pe" | "ap" = "pe";
  private burnErrorPrev = Number.POSITIVE_INFINITY;
  private burnStall = 0;

  private holdTime = 0;
  private complete = false;
  private closed = false;
  private lastNow = 0;
  private menuTime = 0;
  private baseOrientation = new Quaternion();
  private showOrbitLines = true;
  private sensitivity = 1;
  private invertY = false;
  private fps = 60;
  private fpsAccum = 0;
  private fpsFrames = 0;
  private camMode: CameraMode = "flight";
  private dragging = false;
  private lastPointer = { x: 0, y: 0 };
  private readonly keys = new Set<string>();
  private readonly canvas: HTMLCanvasElement;

  constructor() {
    this.canvas = document.getElementById("viewport") as HTMLCanvasElement;
    this.view = new FlightView(this.canvas);
    this.hud = new Hud();
    this.screens = new Screens();

    this.hud.setVisible(false);
    this.hud.onThrottle = (v) => {
      this.throttle = v;
    };
    this.hud.onEngineToggle = () => this.toggleEngine();
    this.hud.onSas = (mode) => this.setSas(mode);
    this.hud.onRcs = () => this.toggleRcs();
    this.hud.onWarp = (n) => this.setWarp(n);
    this.hud.onCamera = (mode) => this.setCamera(mode);
    this.hud.onAddNode = () => this.addNode();
    this.hud.onClearNode = () => this.clearNode();
    this.hud.onNodeDv = (delta) => this.adjustNodeDv(delta);
    this.hud.onExecute = () => (this.autopilot === "idle" ? this.executeNode() : this.cancelAutopilot("AUTOPILOT ABORTED"));
    this.hud.onEndFlight = () => this.finishFlight();

    this.screens.onLaunch = (id) => this.startFlight(id);
    this.screens.onCoreStatus = () => this.coreStatus;
    this.screens.onResume = () => this.resume();
    this.screens.onRestart = () => this.startFlight(this.contract.id);
    this.screens.onReturnToMenu = () => this.returnToMenu();
    this.screens.onQuit = () => this.quit();
    this.screens.onSettingsChanged = (settings) => {
      this.showOrbitLines = settings.orbitLines;
      this.sensitivity = settings.sensitivity;
      this.invertY = settings.invertY;
      this.audio.setEnabled(settings.audio);
      this.view.setOrbitLinesVisible(settings.orbitLines);
    };
    this.showOrbitLines = this.screens.settings.orbitLines;
    this.sensitivity = this.screens.settings.sensitivity;
    this.invertY = this.screens.settings.invertY;
    this.audio.setEnabled(this.screens.settings.audio);

    this.bindInput();
    this.resize();
    this.loadPreview();
    this.lastNow = performance.now();
    requestAnimationFrame(this.loop);
  }

  /* ------------------------------------------------------------------ setup */

  private loadPreview(): void {
    this.contract = CONTRACTS[0]!;
    const state = initialFlightState(this.contract);
    this.baseOrientation = localFrameQuaternion(state.r, state.v);
    this.vessel = new Vessel(KESTREL, state, this.baseOrientation);
    this.target = targetElements(this.contract, this.vessel.elements);
    this.evaluation = evaluate(this.contract, this.vessel.elements);
    this.menuTime = 0;
    this.met = 0;
    this.node = null;
    this.planned = null;
    this.nodePosition = null;
    this.complete = false;
    this.closed = false;
    this.autopilot = "idle";
    this.throttle = 0;
    this.engineArmed = false;
    this.camMode = "flight";
    this.view.setCameraMode("flight");
    this.view.setCameraInput({ yaw: 1.02, pitch: 0.36, distance: 24, cinematic: true });
    this.setSunFromOrbit(state.r, state.v);
    this.view.setOrbitLinesVisible(this.showOrbitLines);
  }

  private setSunFromOrbit(r: Vector3, v: Vector3): void {
    const up = r.clone().normalize();
    const fwd = v.clone().normalize();
    const side = new Vector3().crossVectors(up, fwd).normalize();
    const yaw = 0.98;
    const sun = new Vector3()
      .addScaledVector(side, Math.cos(yaw) * 0.80)
      .addScaledVector(fwd, Math.sin(yaw) * 0.80)
      .addScaledVector(up, 0.52)
      .normalize();
    this.view.setSunDirection(sun);
  }

  /* ------------------------------------------------------------------ input */

  private bindInput(): void {
    window.addEventListener("resize", () => this.resize());
    window.addEventListener("keydown", (ev) => this.onKeyDown(ev));
    window.addEventListener("keyup", (ev) => {
      this.keys.delete(ev.key.toLowerCase());
    });
    window.addEventListener("blur", () => {
      this.keys.clear();
      this.dragging = false;
    });

    this.canvas.addEventListener("contextmenu", (ev) => ev.preventDefault());
    this.canvas.addEventListener("pointerdown", (ev) => {
      this.audio.unlock();
      if (this.mode !== "flight" || this.paused) return;
      this.dragging = true;
      this.lastPointer = { x: ev.clientX, y: ev.clientY };
      this.canvas.setPointerCapture(ev.pointerId);
    });
    this.canvas.addEventListener("pointermove", (ev) => {
      if (!this.dragging) return;
      const dx = ev.clientX - this.lastPointer.x;
      const dy = ev.clientY - this.lastPointer.y;
      this.lastPointer = { x: ev.clientX, y: ev.clientY };
      const cam = this.view.cameraState;
      const scale = 0.0045 * this.sensitivity;
      const pitchDelta = (dy * scale) * (this.invertY ? -1 : 1);
      this.view.setCameraInput({
        yaw: cam.yaw + dx * scale,
        pitch: clamp(cam.pitch + pitchDelta, -0.55, 1.1),
        cinematic: false,
      });
    });
    this.canvas.addEventListener("pointerup", (ev) => {
      this.dragging = false;
      try {
        this.canvas.releasePointerCapture(ev.pointerId);
      } catch {
        /* pointer may already be released */
      }
    });
    this.canvas.addEventListener(
      "wheel",
      (ev) => {
        if (this.mode !== "flight") return;
        ev.preventDefault();
        const cam = this.view.cameraState;
        this.view.setCameraInput({
          distance: clamp(cam.distance * (1 + Math.sign(ev.deltaY) * 0.08), 10, 220),
          cinematic: false,
        });
      },
      { passive: false },
    );
    this.canvas.addEventListener("dblclick", () => {
      if (this.mode !== "flight") return;
      this.view.setCameraInput({ yaw: 1.02, pitch: 0.36, distance: 24, cinematic: false });
    });
  }

  private onKeyDown(ev: KeyboardEvent): void {
    this.audio.unlock();
    if (this.screens.key(ev)) {
      ev.preventDefault();
      return;
    }

    const key = ev.key.toLowerCase();

    if (this.mode === "menu") {
      if (key === "f11") this.toggleFullscreen();
      if (this.screens.overlaysOpen()) return;
      return;
    }

    if (this.mode === "debrief") {
      if (key === "escape") this.returnToMenu();
      return;
    }

    // Flight.
    if (key === "escape") {
      if (this.paused) this.resume();
      else this.pause();
      ev.preventDefault();
      return;
    }
    if (this.paused) return;

    if (["w", "a", "s", "d", "q", "e"].includes(key) && this.autopilot !== "idle") {
      this.cancelAutopilot("MANUAL CONTROL TAKEN");
    }
    this.keys.add(key);

    switch (key) {
      case " ":
        this.toggleEngine();
        ev.preventDefault();
        break;
      case "t":
        this.cycleSas();
        break;
      case "r":
        this.toggleRcs();
        break;
      case "1":
        this.setCamera("flight");
        break;
      case "2":
        this.setCamera("chase");
        break;
      case "3":
        this.setCamera("map");
        break;
      case ".":
        this.setWarp(this.nextWarp(1));
        break;
      case ",":
        this.setWarp(this.nextWarp(-1));
        break;
      case "f11":
        this.toggleFullscreen();
        break;
      default:
        break;
    }
  }

  private nextWarp(dir: number): number {
    const steps = [1, 5, 25, 100];
    const i = steps.indexOf(this.warp);
    const next = clamp((i < 0 ? 0 : i) + dir, 0, steps.length - 1);
    return steps[next] ?? 1;
  }

  /* ------------------------------------------------------------------ flight */

  startFlight(id: string): void {
    this.contract = findContract(id);
    const state = initialFlightState(this.contract);
    this.vessel = new Vessel(KESTREL, state, localFrameQuaternion(state.r, state.v));
    this.target = targetElements(this.contract, this.vessel.elements);
    this.evaluation = evaluate(this.contract, this.vessel.elements);
    this.met = 0;
    this.warp = 1;
    this.throttle = 0;
    this.engineArmed = false;
    this.sas = "prograde";
    this.rcs = false;
    this.node = null;
    this.planned = null;
    this.nodePosition = null;
    this.autopilot = "idle";
    this.complete = false;
    this.closed = false;
    this.holdTime = 0;
    this.mode = "flight";
    this.paused = false;
    this.setSunFromOrbit(state.r, state.v);
    this.view.setCameraMode("flight");
    this.view.setCameraInput({ yaw: 1.02, pitch: 0.36, distance: 24, cinematic: false });
    this.view.setOrbitLinesVisible(this.showOrbitLines);
    this.screens.showFlight();
    this.hud.setVisible(true);
    this.hud.setCamera("flight");
    this.hud.setWarp(1);
    this.hud.setSasMode(this.sas);
    this.refreshNode();
    this.audio.click();
    this.canvas.focus();
  }

  private pause(): void {
    if (this.mode !== "flight") return;
    this.paused = true;
    this.screens.showPause(this.met);
  }

  private resume(): void {
    if (this.mode !== "flight") return;
    this.paused = false;
    this.screens.hidePause();
  }

  private returnToMenu(): void {
    this.mode = "menu";
    this.paused = false;
    this.hud.setVisible(false);
    this.screens.hideOverlays();
    this.screens.hidePause();
    this.screens.hideDebrief();
    this.screens.showMenu();
    this.loadPreview();
  }

  private quit(): void {
    this.audio.click();
    window.close();
    // Browsers refuse window.close() without an opener; return to the menu and
    // tell the operator how to leave the tab.
    window.setTimeout(() => {
      if (this.mode === "menu") {
        this.screens.hideOverlays();
        this.hud.flash("CLOSE THE TAB TO EXIT", "warn", 3500);
      }
    }, 120);
  }

  private finishFlight(): void {
    if (this.mode !== "flight") return;
    const el = this.vessel.elements;
    const result = scoreFlight(
      this.contract,
      el,
      this.vessel.fuelFraction,
      this.vessel.fuelUsed,
      this.met,
    );
    this.mode = "debrief";
    this.paused = true;
    this.autopilot = "idle";
    this.hud.setVisible(false);
    this.screens.showDebrief(result);
    this.view.setCameraInput({ ...this.view.cameraState, cinematic: true });
    this.audio.success();
  }

  /* ------------------------------------------------------------------ systems */

  private toggleEngine(): void {
    if (this.mode !== "flight" || this.paused) return;
    if (this.autopilot !== "idle") this.cancelAutopilot("AUTOPILOT ABORTED");
    this.engineArmed = !this.engineArmed;
    if (this.engineArmed && this.throttle <= 0.01) this.throttle = 0.6;
    this.audio.blip(this.engineArmed ? 520 : 300, 60, "square", 0.05);
  }

  private setSas(mode: SasMode): void {
    if (this.mode !== "flight") return;
    if (this.autopilot !== "idle" && mode !== "node") this.cancelAutopilot("AUTOPILOT ABORTED");
    this.sas = mode;
    this.vessel.sas = mode;
    if (mode !== "node") this.vessel.targetAttitude = null;
    this.hud.setSasMode(mode);
    this.audio.click();
  }

  private cycleSas(): void {
    const modes: SasMode[] = ["off", "stability", "prograde", "retrograde", "radial", "normal"];
    const i = modes.indexOf(this.sas);
    this.setSas(modes[(i + 1) % modes.length] ?? "prograde");
  }

  private toggleRcs(): void {
    if (this.mode !== "flight") return;
    this.rcs = !this.rcs;
    this.vessel.rcs = this.rcs;
    this.audio.click();
  }

  private setWarp(n: number): void {
    if (this.mode !== "flight") return;
    const engineOn = this.engineArmed && this.throttle > 0.001;
    this.warp = engineOn ? Math.min(Math.max(1, n), 5) : Math.max(1, n);
    this.hud.setWarp(this.warp);
    this.audio.blip(this.warp > 1 ? 880 + this.warp * 4 : 660, 40, "sine", 0.03);
  }

  private setCamera(mode: CameraMode): void {
    this.camMode = mode;
    this.view.setCameraMode(mode);
    this.hud.setCamera(mode);
    this.audio.click();
  }

  private toggleFullscreen(): void {
    if (document.fullscreenElement) void document.exitFullscreen();
    else void document.documentElement.requestFullscreen().catch(() => undefined);
  }

  /* -------------------------------------------------------------- manoeuvres */

  private addNode(): void {
    if (this.mode !== "flight") return;
    const el = this.vessel.elements;
    if (!el.closed) {
      this.hud.flash("NO CLOSED ORBIT TO PLAN FROM", "warn");
      return;
    }
    const mu = EARTH_MU;
    const targetPe = EARTH_RADIUS + this.contract.target.peAlt;
    const targetAp = EARTH_RADIUS + this.contract.target.apAlt;
    const tolerance = 45_000;

    const raisePe = targetPe - el.pe > tolerance;
    const lowerAp = !raisePe && el.ap - targetAp > tolerance;
    if (!raisePe && !lowerAp) {
      this.hud.flash("ORBIT ALREADY WITHIN TOLERANCE", "good");
      return;
    }

    // Burn at the apsis we want to move: apoapsis to raise periapsis,
    // periapsis to lower apoapsis. Both are optimal for a tangential burn.
    const atApo = raisePe;
    let wait = timeToApsis(el, atApo);
    if (!Number.isFinite(wait)) wait = 120;
    // Never park the node half an orbit away: if we are already at the right
    // apsis, burn on the next pass through it after a short settling coast.
    if (wait < 20) wait += el.period || 3600;
    const rBurn = atApo ? el.ap : el.pe;
    const aWant = atApo ? (el.ap + targetPe) / 2 : (el.pe + targetAp) / 2;
    const vNow = Math.sqrt(mu * (2 / rBurn - 1 / el.a));
    const vWant = Math.sqrt(mu * (2 / rBurn - 1 / aWant));
    const dv = clamp(vWant - vNow, -1500, 1500);

    this.node = { dv, time: this.met + wait };
    this.refreshNode();
    this.hud.flash(
      `${atApo ? "APOAPSIS" : "PERIAPSIS"} NODE · ${Math.abs(dv).toFixed(0)} M/S`,
      "good",
    );
    this.audio.click();
  }

  private clearNode(): void {
    if (this.autopilot !== "idle") this.cancelAutopilot();
    this.node = null;
    this.planned = null;
    this.nodePosition = null;
    this.audio.click();
  }

  private adjustNodeDv(delta: number): void {
    if (this.mode !== "flight") return;
    if (!this.node) return;
    this.node.dv = clamp(this.node.dv + delta, -1200, 1500);
    this.refreshNode();
  }

  private refreshNode(): void {
    if (!this.node) {
      this.planned = null;
      this.nodePosition = null;
      return;
    }
    const el = this.vessel.elements;
    const wait = Math.max(0, this.node.time - this.met);
    const at = propagateKepler(this.vessel.state.r, this.vessel.state.v, wait);
    const vHat = at.v.clone().normalize();
    const v2 = at.v.clone().addScaledVector(vHat, this.node.dv);
    this.planned = elementsFromState(at.r, v2);
    this.nodePosition = at.r.clone();
    void el;
  }

  private burnTimeFor(dv: number): number {
    const mdot = KESTREL.thrust / (KESTREL.isp * G0);
    const m0 = this.vessel.mass;
    const m1 = m0 * Math.exp(-Math.abs(dv) / (KESTREL.isp * G0));
    return Math.max(0, (m0 - m1) / mdot);
  }

  private executeNode(): void {
    if (!this.node || Math.abs(this.node.dv) < 1) {
      this.hud.flash("NO MANOEUVRE LOADED", "warn");
      return;
    }
    const el = this.vessel.elements;
    const targetPe = EARTH_RADIUS + this.contract.target.peAlt;
    this.burnObjective = targetPe - el.pe > EARTH_RADIUS + this.contract.target.apAlt - el.ap ? "pe" : "ap";
    this.burnSign = this.burnObjective === "pe" ? 1 : -1;
    this.burnDone = 0;
    this.burnDuration = this.burnTimeFor(this.node.dv);
    this.burnNodeTime = this.node.time;
    this.autopilot = "waiting";
    this.sas = "node";
    this.vessel.sas = "node";
    this.hud.setSasMode("node");
    const tte = this.burnNodeTime - this.met;
    if (tte > 600) this.setWarp(100);
    else if (tte > 90) this.setWarp(25);
    this.hud.flash("AUTOPILOT ARMED", "good");
    this.audio.click();
  }

  private cancelAutopilot(message?: string): void {
    this.autopilot = "idle";
    this.engineArmed = false;
    this.throttle = 0;
    this.vessel.targetAttitude = null;
    this.sas = "stability";
    this.vessel.sas = "stability";
    this.hud.setSasMode("stability");
    if (message) this.hud.flash(message, "warn");
  }

  private autopilotTick(dtWall: number): void {
    if (this.autopilot === "idle" || !this.node) return;
    const wait = this.burnNodeTime - this.met;
    const at = propagateKepler(this.vessel.state.r, this.vessel.state.v, Math.max(0, wait));
    const fwd = at.v.clone().normalize();
    const dir = this.burnSign >= 0 ? fwd : fwd.clone().multiplyScalar(-1);
    const err = this.vessel.alignError();
    this.vessel.targetAttitude = basisQuaternion(dir, at.r.clone().normalize());
    this.sas = "node";
    this.vessel.sas = "node";

    if (this.autopilot === "waiting") {
      if (wait < 120 && this.warp > 5) this.setWarp(5);
      if (wait <= this.burnDuration / 2 + 14) {
        this.autopilot = "aligning";
        if (this.warp > 1) this.setWarp(1);
        this.hud.flash("ALIGNING FOR BURN", "good");
      }
    }
    if (this.autopilot === "aligning") {
      // Ignition is centred on the node: charge half the burn time before it.
      const ignition = this.burnDuration * 0.5;
      if (err < 0.06 && (wait <= ignition || wait <= -1)) {
        this.autopilot = "burning";
        this.engineArmed = true;
        this.throttle = 1;
        this.burnErrorPrev = Number.POSITIVE_INFINITY;
        this.burnStall = 0;
        this.hud.flash("BURN IN PROGRESS", "good");
      }
    }
    if (this.autopilot === "burning") {
      // Closed-loop guidance: keep burning until the *measured* orbit matches
      // the target, not until a pre-computed stopwatch runs out. Finite burns
      // over minutes would otherwise miss by hundreds of kilometres.
      const targetPe = EARTH_RADIUS + this.contract.target.peAlt;
      const targetAp = EARTH_RADIUS + this.contract.target.apAlt;
      const el = this.vessel.elements;
      const error = this.burnObjective === "pe" ? targetPe - el.pe : el.ap - targetAp;
      // The contract tolerance is ±45 km; stop with margin so the residual
      // finite-burn drift still lands inside it.
      const margin = Math.max(6_000, this.contract.target.tolerance * 0.55);
      const peOk = Math.abs(el.pe - targetPe) <= margin;
      const apOk = Math.abs(el.ap - targetAp) <= margin;
      const burnedFor = this.met - (this.burnNodeTime - this.burnDuration * 0.5);
      const overrun = burnedFor > this.burnDuration * 2.4;
      if ((peOk && apOk) || error <= 400 || overrun || this.vessel.propellant <= 0) {
        this.finishBurn();
        return;
      }
      // If the targeted apsis has stopped improving, more thrust only makes the
      // other one worse: cut the engine and let the operator trim.
      if (error < this.burnErrorPrev - 250) this.burnStall = 0;
      else this.burnStall += dtWall;
      this.burnErrorPrev = error;
      if (this.burnStall > 4) {
        this.hud.flash("BURN CUT · TRIM REQUIRED", "warn");
        this.finishBurn();
        return;
      }
      this.throttle = clamp(error / 30_000, 0.22, 1);
      this.engineArmed = true;
    }
  }

  private finishBurn(): void {
    this.throttle = 0;
    this.engineArmed = false;
    this.autopilot = "complete";
    this.vessel.sas = "stability";
    this.vessel.targetAttitude = null;
    this.sas = "stability";
    this.hud.setSasMode("stability");
    this.hud.flash("BURN COMPLETE", "good");
    this.audio.success();
    // The plan is spent. Leaving it loaded would draw a stale conic and show a
    // prediction for a burn that has already happened.
    this.node = null;
    this.planned = null;
    this.nodePosition = null;
    window.setTimeout(() => {
      if (this.autopilot === "complete") this.autopilot = "idle";
    }, 1800);
  }

  private accumulateBurn(dt: number): void {
    if (this.autopilot !== "burning") return;
    this.burnDone += (this.vessel.thrust / Math.max(this.vessel.mass, 1)) * dt;
    // Hard stop if guidance is somehow starved (e.g. no propellant flow).
    if (this.vessel.propellant <= 0) this.finishBurn();
  }

  /* ------------------------------------------------------------------- loop */

  private loop = (now: number): void => {
    try {
      this.frame(now);
    } catch (error) {
      // A single bad frame must never stop the flight computer.
      console.error("frame error", error);
    }
    requestAnimationFrame(this.loop);
  };

  private frame(now: number): void {
    const dtRaw = (now - this.lastNow) / 1000;
    this.lastNow = now;
    const dt = clamp(Number.isFinite(dtRaw) ? dtRaw : 0.016, 0.0001, 0.1);
    this.fpsAccum += dt;
    this.fpsFrames += 1;
    if (this.fpsAccum > 0.5) {
      this.fps = Math.round(this.fpsFrames / this.fpsAccum);
      this.fpsAccum = 0;
      this.fpsFrames = 0;
    }

    // Attitude and animation integrate on the clamped step; the mission clock
    // has to follow real elapsed time or high warp silently runs slow.
    const simDt = clamp(Number.isFinite(dtRaw) ? dtRaw : 0.016, 0.0001, 0.5);
    if (this.mode === "menu") this.menuTick(dt, simDt);
    else if (this.mode === "flight") this.flightTick(dt, simDt);
    else this.debriefTick(dt);

    this.view.update(dt, {
      elements: this.vessel.elements,
      position: this.vessel.state.r,
      velocity: this.vessel.state.v,
      orientationQuaternion: this.vessel.orientation,
      throttle: this.throttle,
      actualThrottle: this.mode === "menu" ? 0 : this.vessel.spool,
      gimbal: this.vessel.gimbal,
      planned: this.autopilot === "burning" ? this.vessel.elements : this.planned,
      target: this.target,
      nodePosition: this.nodePosition,
      showTarget: this.showOrbitLines,
      time: this.met,
      mode: this.camMode,
      paused: this.paused || this.mode !== "flight",
    });
    this.view.render();
  }

  private menuTick(dt: number, simDt: number): void {
    this.menuTime += dt;
    this.met += simDt * 8;
    const st = propagateKepler(this.vessel.state.r, this.vessel.state.v, simDt * 8);
    this.vessel.state = st;
    // A slow roll keeps the plate alive without reading as a turntable.
    const roll = new Quaternion().setFromAxisAngle(new Vector3(0, 1, 0), Math.sin(this.menuTime * 0.18) * 0.55 + 0.2);
    this.vessel.orientation.copy(this.baseOrientation).multiply(roll);
    this.vessel.sas = "off";
    this.vessel.update(dt, { pitch: 0, yaw: 0, roll: 0, throttle: 0, engineArmed: false });
    this.vessel.state = st;
    if (this.menuTime > 120) this.menuTime = 0;
  }

  private flightTick(dt: number, simDt: number): void {
    if (this.paused) {
      this.view.setCameraInput({ ...this.view.cameraState, cinematic: false });
      return;
    }
    this.autopilotTick(dt);
    this.vessel.rcs = this.rcs;

    // Continuous throttle keys (processed before the physics step).
    const fine = this.keys.has("shift");
    const rate = (fine ? 0.12 : 0.55) * dt;
    if (this.keys.has("z")) this.throttle = clamp(this.throttle + rate, 0, 1);
    if (this.keys.has("x")) this.throttle = clamp(this.throttle - rate, 0, 1);
    if (this.autopilot === "burning") this.throttle = this.throttle;

    if (this.engineArmed && this.vessel.propellant <= 0) {
      this.engineArmed = false;
      this.throttle = 0;
    }

    const engineOn = this.engineArmed && this.throttle > 0.001 && this.vessel.propellant > 0;
    if (engineOn) {
      let remaining = simDt * Math.min(this.warp, 5);
      let guard = 0;
      while (remaining > 1e-6 && guard++ < 400) {
        const step = Math.min(remaining, 0.05);
        const input: VesselInput = this.vesselInput();
        const accel = this.vessel.update(step, input);
        const next = stepRK4(this.vessel.state.r, this.vessel.state.v, step, { accel, j2: true });
        this.vessel.state = next;
        this.met += step;
        this.accumulateBurn(step);
        remaining -= step;
        if (!(this.engineArmed && this.throttle > 0.001)) break;
      }
    } else {
      this.vessel.update(dt, { ...this.vesselInput(), throttle: 0 });
      const secs = simDt * this.warp;
      // Kepler is analytic, but very long single steps lose precision near
      // periapsis; propagate in bounded slices instead.
      let left = secs;
      while (left > 1e-6) {
        const step = Math.min(left, Math.max(120, secs / 8));
        const next = propagateKepler(this.vessel.state.r, this.vessel.state.v, step);
        this.vessel.state = next;
        this.met += step;
        left -= step;
      }
    }

    this.evaluation = evaluate(this.contract, this.vessel.elements);
    this.closed = this.met > this.contract.window;
    if (!this.paused && this.evaluation.inTolerance && !this.closed) {
      this.holdTime += dt;
      if (this.holdTime > 5 && !this.complete) {
        this.complete = true;
        this.hud.flash("OBJECTIVE SECURED · END FLIGHT TO FILE", "good", 5200);
        this.audio.success();
      }
    } else if (!this.evaluation.inTolerance) {
      this.holdTime = 0;
    }

    if (this.throttle > 0 && this.engineArmed) {
      // Keep warping usable but honest about attitude authority.
      if (this.warp > 5) this.setWarp(5);
    }

    this.audio.setThrottle(this.vessel.spool * (this.paused ? 0 : 1));
    this.refreshNodeTimer(dt);
    this.updateHud();
  }

  private debriefTick(dt: number): void {
    this.menuTime += dt;
    const cam = this.view.cameraState;
    this.view.setCameraInput({
      yaw: 0.98 + Math.sin(this.menuTime * 0.09) * 0.16,
      pitch: 0.36,
      distance: cam.distance,
      cinematic: false,
    });
  }

  private updateHud(): void {
    const data: HudData = {
      contract: this.contract,
      elements: this.vessel.elements,
      target: this.target,
      evaluation: this.evaluation,
      met: this.met,
      warp: this.warp,
      fps: this.fps,
      throttle: this.vessel.throttle,
      armed: this.engineArmed,
      sas: this.sas,
      rcs: this.rcs,
      propellant: this.vessel.propellant,
      fuelFraction: this.vessel.fuelFraction,
      mass: this.vessel.mass,
      thrust: this.vessel.thrust,
      deltaV: this.vessel.deltaVRemaining,
      attitude: attitudeInfo(this.vessel.orientation, this.vessel.state.r, this.vessel.state.v),
      orientation: this.vessel.orientation,
      position: this.vessel.state.r,
      velocity: this.vessel.state.v,
      node: this.node
        ? {
            active: true,
            dv: this.node.dv,
            time: this.node.time,
            burnTime: this.burnTimeFor(this.node.dv),
            planned: this.autopilot === "burning" ? this.vessel.elements : this.planned,
          }
        : null,
      autopilot: this.autopilot,
      complete: this.complete,
      closed: this.closed,
      paused: this.paused,
    };
    this.hud.update(data);
    this.hud.setMet(this.met);
  }

  private planAccum = 0;

  private refreshNodeTimer(dt: number): void {
    this.planAccum += dt;
    if (!this.node) {
      this.planned = null;
      this.nodePosition = null;
      return;
    }
    if (this.planAccum < 0.15 && this.planned) return;
    this.planAccum = 0;
    this.refreshNode();
  }

  private vesselInput(): VesselInput {
    let pitch = 0;
    let yaw = 0;
    let roll = 0;
    if (this.autopilot === "idle") {
      pitch = (this.keys.has("w") ? 1 : 0) - (this.keys.has("s") ? 1 : 0);
      yaw = (this.keys.has("a") ? 1 : 0) - (this.keys.has("d") ? 1 : 0);
      roll = (this.keys.has("q") ? 1 : 0) - (this.keys.has("e") ? 1 : 0);
    }
    return { pitch, yaw, roll, throttle: this.throttle, engineArmed: this.engineArmed };
  }

  private resize(): void {
    const w = window.innerWidth;
    const h = window.innerHeight;
    this.view.resize(w, h);
    this.hud.resize();
  }
}
