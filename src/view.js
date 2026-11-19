import * as T from "three";
import { Ship } from "./ship.js";
import { clamp, mulberry32 } from "./math.js";

// An illustrative Earth, not an ephemeris or a map of real coastlines. Surface,
// weather, atmospheric limb, starlight and vehicle materials are drawn locally.
const rnd = mulberry32(419),
  noise = Float32Array.from({ length: 65536 }, rnd);
function n(x, y) {
  const i = Math.floor(x),
    j = Math.floor(y);
  let a = x - i,
    b = y - j;
  a *= a * (3 - 2 * a);
  b *= b * (3 - 2 * b);
  const k = (i + j * 2311) & 65535;
  return (
    (1 - b) * (noise[k] + a * (noise[(k + 1) & 65535] - noise[k])) +
    b *
      (noise[(k + 2311) & 65535] +
        a * (noise[(k + 2312) & 65535] - noise[(k + 2311) & 65535]))
  );
}
function fb(x, y, oct = 5) {
  let s = 0,
    a = 0.52;
  for (let i = 0; i < oct; i++) {
    s += a * n(x, y);
    const t = x;
    x = 1.72 * x + 0.98 * y + 13.4;
    y = 1.72 * y - 0.98 * t + 2.2;
    a *= 0.5;
  }
  return s;
}
function sky(canvas, phase, altitude, angle) {
  const g = canvas.getContext("2d", { alpha: false }),
    w = canvas.width,
    h = canvas.height;
  const image = g.createImageData(w, h),
    px = image.data;
  const rad = w * 1.48,
    cx = w * (1.02 + 0.07 * Math.sin(angle)),
    cy = h * 0.52 + rad + Math.min(0.25, altitude / 4e7) * h;
  const stars = mulberry32(2025);
  for (let j = 0; j < h; j++) {
    for (let i = 0; i < w; i++) {
      const k = 4 * (i + j * w),
        x = (i - cx) / rad,
        y = (j - cy) / rad,
        d = x * x + y * y,
        limb = (Math.sqrt(d) - 1) * rad;
      let r = 4,
        b = 13,
        green = 8;
      if (limb < 0) {
        const z = Math.sqrt(Math.max(0, 1 - d)),
          u = 32 * Math.atan2(x, z) + phase,
          v = 22 * Math.asin(y);
        const a = fb(u * 0.27, v * 0.27),
          b1 = fb(u * 0.35 + 4, v * 0.4 - 11);
        const land = fb(u * 0.7 + a * 3, v * 0.7 + b1 * 2);
        const turb = fb(u * 2 + 3 * a, v * 2 + b1 * 3),
          streak = fb(u * 2.3 + 8 * b1 + 3 * turb, v * 4.8 + 9 * a + 2 * turb);
        const fine = n(u * 160, v * 200);
        const clouds = clamp((streak + 0.23 * turb - 0.53) * 3.9, 0, 0.94);
        const rough = 22 * turb + 8 * fine;
        const coast = clamp((land - 0.51) * 23, 0, 1);
        r = 10 + coast * (51 + rough);
        green = 38 + coast * (36 + rough);
        b = 62 + coast * (11 + rough * 0.6);
        const shade = 0.49 + 0.52 * (i / w);
        const whiten = 151 + 67 * fb(2 * u, 3 * v) + 25 * fine;
        r = (r * (1 - clouds) + whiten * clouds) * shade;
        green = (green * (1 - clouds) + (whiten + 10) * clouds) * shade;
        b = (b * (1 - clouds) + (whiten + 24) * clouds) * shade;
        const haze = 0.38 * Math.exp(-z * 5);
        r += haze * 85;
        green += haze * 134;
        b += haze * 173;
      } else {
        const dust = n(i * 0.006, j * 0.008) * 4;
        const band = Math.exp(
          -Math.pow((j / h - 0.17 - (0.28 * i) / w) * 6, 2),
        );
        r += dust * band;
        green += dust * band * 1.15;
        b += dust * band * 1.9;
      }
      const glow =
          Math.exp(-Math.max(0, limb) / 9) * Math.exp(Math.min(0, limb) / 9),
        edge = Math.exp(-Math.abs(limb) / 1.4);
      r += glow * 33 + edge * 93;
      green += glow * 80 + edge * 118;
      b += glow * 156 + edge * 100;
      px[k] = r;
      px[k + 1] = green;
      px[k + 2] = b;
      px[k + 3] = 255;
    }
  }
  g.putImageData(image, 0, 0);
  for (let k = 0; k < (w * h) / 270; k++) {
    const x = stars() * w,
      y = stars() * h;
    const limb = Math.hypot(x - cx, y - cy) - rad;
    if (limb < 5) continue;
    const a = stars(),
      s = stars();
    g.fillStyle = `rgba(${s > 0.88 ? "194,216,250" : "220,225,226"},${0.1 + a * a * 0.65})`;
    g.fillRect(
      x,
      y,
      s > 0.97 ? 1.8 : 0.5 + a * 0.5,
      s > 0.97 ? 1.8 : 0.5 + a * 0.5,
    );
    if (s > 0.996) {
      g.globalAlpha = 0.28;
      g.fillRect(x - 2, y + 0.5, 6, 0.7);
      g.fillRect(x + 0.5, y - 2, 0.7, 6);
      g.globalAlpha = 1;
    }
  }
}
export class View {
  constructor(container) {
    this.el = container;
    this.back = container.querySelector("#universe");
    this.fg = container.querySelector("#spacecraft");
    this.map = container.querySelector("#map");
    this.zoom = 1;
    this.yaw = 0;
    this.roll = 0;
    this.lastSky = "";
    this.mode = "external";
    this.hud = true;
    try {
      this.renderer = new T.WebGLRenderer({
        canvas: this.fg,
        alpha: true,
        antialias: true,
        powerPreference: "low-power",
      });
      this.renderer.setPixelRatio(Math.min(devicePixelRatio, 1.7));
      this.renderer.setClearColor(0, 0);
      this.renderer.toneMapping = T.ACESFilmicToneMapping;
      this.scene = new T.Scene();
      this.camera = new T.PerspectiveCamera(38, 1, 0.1, 200);
      this.camera.position.set(0, 0, 25);
      this.ship = new Ship();
      this.scene.add(this.ship.root);
      this.scene.add(new T.HemisphereLight(0xe1eeff, 0x526581, 1.8));
      const sun = new T.DirectionalLight(0xffebce, 3.3);
      sun.position.set(3, 7, 6);
      this.scene.add(sun);
      const rim = new T.DirectionalLight(0x82a9e2, 2.2);
      rim.position.set(-3, 3, -4);
      this.scene.add(rim);
    } catch {
      this.fg.hidden = true;
      this.el.classList.add("vector-mode");
    }
    new ResizeObserver(() => this.draw(this.last, true)).observe(container);
    let drag;
    container.addEventListener("pointerdown", (e) => {
      if (e.target.closest("button,.brief,dialog") || this.mode === "map")
        return;
      drag = [e.clientX, e.clientY];
      container.setPointerCapture(e.pointerId);
    });
    container.addEventListener("pointermove", (e) => {
      if (!drag) return;
      this.yaw += (e.clientX - drag[0]) * 0.009;
      this.roll = clamp(this.roll + (e.clientY - drag[1]) * 0.007, -1.2, 1.2);
      drag = [e.clientX, e.clientY];
      this.draw(this.last);
    });
    container.addEventListener("pointerup", () => (drag = null));
    container.addEventListener("pointercancel", () => (drag = null));
    container.addEventListener(
      "wheel",
      (e) => {
        e.preventDefault();
        this.magnify(e.deltaY < 0 ? 1.1 : 1 / 1.1);
      },
      { passive: false },
    );
  }
  magnify(k) {
    this.zoom = clamp(this.zoom * k, 0.6, 2.2);
    this.draw(this.last);
  }
  draw(s, resize = false) {
    if (!s) return;
    this.last = s;
    const w = this.el.clientWidth,
      h = this.el.clientHeight;
    if (!w || !h) return;
    if (resize || this.width !== w || this.height !== h) {
      this.width = w;
      this.height = h;
      this.back.width = Math.min(1600, w);
      this.back.height = Math.round((this.back.width * h) / w);
      this.map.width = w;
      this.map.height = h;
      if (this.renderer) {
        this.renderer.setSize(w, h, false);
        this.camera.aspect = w / h;
        this.camera.updateProjectionMatrix();
      }
      this.lastSky = "";
    }
    const map = this.mode === "map";
    this.map.hidden = !map;
    this.fg.hidden = map || !this.renderer;
    this.back.hidden = map;
    this.el.classList.toggle("map-mode", map);
    if (map) {
      drawMap(this.map, s, this.zoom);
      return;
    }
    const phase = Math.atan2(s[2], s[1]);
    const key = `${Math.round(phase * 8)},${Math.round(s[8] / 80000)},${Math.round(this.yaw * 2)}`;
    if (this.lastSky !== key) {
      sky(this.back, phase * 3, s[8], this.yaw);
      this.lastSky = key;
    }
    if (this.renderer) {
      this.camera.position.z = 24 / this.zoom;
      this.camera.setViewOffset(
        w,
        h,
        -w * (w > 720 ? 0.075 : 0.085),
        -h * 0.025,
        w,
        h,
      ); // Camera mount followed by yaw about dorsal, pitch about wing, roll about nose.
      this.ship.root.quaternion
        .setFromEuler(
          new T.Euler(-0.47 + this.roll, -0.48 + this.yaw, -0.99, "ZXY"),
        )
        .multiply(
          new T.Quaternion().setFromEuler(
            new T.Euler(s[13], s[24], s[14], "ZXY"),
          ),
        );
      this.ship.update(0.05, s.throttle || 0);
      this.renderer.render(this.scene, this.camera);
    } else {
      const g = this.back.getContext("2d");
      this.lastSky = "";
      vectorVessel(g, w * 0.56, h * 0.52, (this.zoom * h) / 280, -0.55 + s[13]);
    }
  }
}
function vectorVessel(g, x, y, k, a) {
  g.save();
  g.translate(x, y);
  g.scale(k, k);
  g.rotate(a);
  g.fillStyle = "#a18c60";
  g.fillRect(-4, -39, 8, 78);
  for (const p of [-55, 15]) {
    g.fillStyle = "#233a65";
    g.strokeStyle = "#b8b8a0";
    g.fillRect(-16, p, 32, 40);
    g.strokeRect(-16, p, 32, 40);
    for (let j = 4; j < 40; j += 5) {
      g.beginPath();
      g.moveTo(-16, p + j);
      g.lineTo(16, p + j);
      g.stroke();
    }
  }
  const grad = g.createLinearGradient(0, -15, 0, 15);
  grad.addColorStop(0, "#fff5d5");
  grad.addColorStop(0.5, "#b3aaa0");
  grad.addColorStop(1, "#3e4245");
  g.fillStyle = grad;
  g.beginPath();
  g.moveTo(-43, -10);
  g.lineTo(-30, -15);
  g.lineTo(30, -15);
  g.lineTo(52, -7);
  g.lineTo(57, 0);
  g.lineTo(52, 7);
  g.lineTo(30, 15);
  g.lineTo(-30, 15);
  g.lineTo(-43, 10);
  g.fill();
  g.fillStyle = "#31383c";
  g.fillRect(-22, -15, 4, 30);
  g.fillRect(15, -15, 5, 30);
  g.fillRect(-52, -7, 10, 14);
  g.restore();
}
function drawMap(canvas, s, zoom) {
  const g = canvas.getContext("2d"),
    w = canvas.width,
    h = canvas.height,
    cx = w * 0.56,
    cy = h * 0.52;
  g.fillStyle = "#080f16";
  g.fillRect(0, 0, w, h);
  g.strokeStyle = "#17252c";
  for (let i = 0; i < w; i += 32) {
    g.beginPath();
    g.moveTo(i, 0);
    g.lineTo(i, h);
    g.stroke();
  }
  for (let j = 0; j < h; j += 32) {
    g.beginPath();
    g.moveTo(0, j);
    g.lineTo(w, j);
    g.stroke();
  }
  const R = 6378137,
    ap = s[10] + R,
    pe = s[11] + R;
  if (!Number.isFinite(ap) || !Number.isFinite(pe)) {
    g.fillStyle = "#cfb590";
    g.font = "12px monospace";
    g.fillText("OPEN TRAJECTORY · NO BOUND APSIDES", w * 0.37, h * 0.56);
    return;
  }
  const a = (ap + pe) / 2,
    b = Math.sqrt(ap * pe),
    scale = (h * 0.32 * zoom) / ap,
    ang = s[23] || 0;
  g.save();
  g.translate(cx, cy);
  const grad = g.createRadialGradient(
    -R * 0.35 * scale,
    -R * 0.35 * scale,
    0,
    0,
    0,
    R * scale,
  );
  grad.addColorStop(0, "#244b66");
  grad.addColorStop(1, "#102435");
  g.fillStyle = grad;
  g.beginPath();
  g.arc(0, 0, R * scale, 0, Math.PI * 2);
  g.fill();
  g.strokeStyle = "#45667b";
  g.stroke();
  g.strokeStyle = "#f5ad66";
  g.lineWidth = 1.6;
  g.beginPath();
  g.ellipse(-(a - pe) * scale, 0, a * scale, b * scale, 0, 0, 2 * Math.PI);
  g.stroke();
  const radius = Math.hypot(...s.slice(1, 4)); // Project the measured trail and the planning instrument's ECI samples
  // into the osculating periapsis frame. The plotted reference never drives thrust.
  const rh = new T.Vector3(...s.slice(1, 4)).normalize(),
    nh = new T.Vector3()
      .crossVectors(rh, new T.Vector3(...s.slice(4, 7)))
      .normalize(),
    th = new T.Vector3().crossVectors(nh, rh);
  const ph = rh
      .clone()
      .multiplyScalar(Math.cos(ang))
      .addScaledVector(th, -Math.sin(ang)),
    qh = rh
      .clone()
      .multiplyScalar(Math.sin(ang))
      .addScaledVector(th, Math.cos(ang));
  function path(points, color, dash, skip) {
    g.strokeStyle = color;
    g.lineWidth = 1.4;
    g.setLineDash(dash);
    g.beginPath();
    points?.forEach((p, i) => {
      const v = new T.Vector3(p[skip], p[skip + 1], p[skip + 2]),
        x = v.dot(ph) * scale,
        y = -v.dot(qh) * scale;
      i ? g.lineTo(x, y) : g.moveTo(x, y);
    });
    g.stroke();
    g.setLineDash([]);
  }
  path(s.reference, "#91b3d0", [3, 5], 0);
  path(s.track, "#a3dbac", [], 1);
  g.fillStyle = "#fff";
  g.beginPath();
  g.arc(
    Math.cos(ang) * radius * scale,
    -Math.sin(ang) * radius * scale,
    4,
    0,
    7,
  );
  g.fill();
  g.font = "10px monospace";
  g.fillText("AP", -ap * scale - 22, 0);
  g.fillText("PE", pe * scale + 8, 0);
  g.textAlign = "center";
  g.fillStyle = "#9bb3c2";
  g.fillText("EARTH", 0, 4);
  g.restore();
  g.font = "9px monospace";
  g.fillStyle = "#f5ad66";
  g.fillText("─  OSCULATING", 25, h - 58);
  g.fillStyle = "#a3dbac";
  g.fillText("─  FLOWN", 125, h - 58);
  if (s.reference?.length) {
    g.fillStyle = "#91b3d0";
    g.fillText("╌  PLANNED", 200, h - 58);
  }
}
