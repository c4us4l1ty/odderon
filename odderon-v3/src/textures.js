// Procedural art adapted from the Odderon 2 render library.
import { CanvasTexture, SRGBColorSpace } from "three";
import { mulberry32 } from "./math.js";
function canvas(size) {
  const c = document.createElement("canvas");
  c.width = size;
  c.height = size;
  const ctx = c.getContext("2d");
  if (!ctx) throw new Error("2D canvas is unavailable");
  return [c, ctx];
}
function finish(c, srgb = true) {
  const t = new CanvasTexture(c);
  if (srgb) t.colorSpace = SRGBColorSpace;
  t.anisotropy = 8;
  t.needsUpdate = true;
  return t;
}
export function hullTexture() {
  const [c, g] = canvas(1024);
  const rnd = mulberry32(3539);
  g.fillStyle = "#e7e4db";
  g.fillRect(0, 0, 1024, 1024);
  for (let i = 0; i < 90; i++) {
    const x = rnd() * 1024;
    const y = rnd() * 1024;
    const r = 40 + rnd() * 220;
    const grad = g.createRadialGradient(x, y, 0, x, y, r);
    const tone = rnd() > 0.5 ? "255,255,250" : "176,174,164";
    grad.addColorStop(0, `rgba(${tone},${0.05 + rnd() * 0.07})`);
    grad.addColorStop(1, "rgba(0,0,0,0)");
    g.fillStyle = grad;
    g.fillRect(x - r, y - r, r * 2, r * 2);
  }
  g.lineWidth = 2;
  g.strokeStyle = "rgba(78,80,78,0.30)";
  for (let x = 0; x <= 1024; x += 128) {
    g.beginPath();
    g.moveTo(x, 0);
    g.lineTo(x, 1024);
    g.stroke();
  }
  g.strokeStyle = "rgba(78,80,78,0.22)";
  for (let y = 0; y <= 1024; y += 64) {
    g.beginPath();
    g.moveTo(0, y);
    g.lineTo(1024, y);
    g.stroke();
  }
  g.fillStyle = "rgba(92,92,88,0.42)";
  for (let y = 16; y < 1024; y += 64) {
    for (let x = 16; x < 1024; x += 32) {
      g.beginPath();
      g.arc(x, y, 1.8, 0, Math.PI * 2);
      g.fill();
    }
  }
  g.fillStyle = "#13171a";
  g.fillRect(0, 300, 1024, 76);
  g.fillRect(0, 470, 1024, 34);
  g.fillStyle = "#f0eee6";
  g.fillRect(0, 322, 1024, 8);
  g.strokeStyle = "rgba(70,72,70,0.35)";
  g.lineWidth = 1.5;
  for (const [x, y, w, h] of [
    [96, 150, 120, 70],
    [330, 170, 90, 54],
    [640, 140, 140, 84],
    [830, 430, 120, 60],
    [140, 640, 110, 54],
  ]) {
    g.strokeRect(x, y, w, h);
    g.beginPath();
    for (let i = 1; i < 4; i++) {
      g.moveTo(x + 6, y + (h * i) / 4);
      g.lineTo(x + w - 6, y + (h * i) / 4);
    }
    g.stroke();
  }
  for (const [x, y] of [
    [220, 905],
    [452, 920],
    [700, 900],
    [860, 930],
  ]) {
    g.save();
    g.translate(x, y);
    g.fillStyle = "rgba(200,121,42,0.85)";
    g.beginPath();
    g.moveTo(0, -11);
    g.lineTo(11, 9);
    g.lineTo(-11, 9);
    g.closePath();
    g.fill();
    g.fillStyle = "#1d1c18";
    g.font = "700 12px ui-monospace, monospace";
    g.textAlign = "center";
    g.fillText("!", 0, 6);
    g.restore();
  }
  g.save();
  g.translate(512, 640);
  g.fillStyle = "rgba(22,26,28,0.85)";
  g.font = "700 46px ui-monospace, monospace";
  g.textAlign = "center";
  g.letterSpacing = "14px";
  g.fillText("ODDERON", 0, 0);
  g.font = "500 22px ui-monospace, monospace";
  g.fillText("KESTREL \xB7 K-9 \xB7 FLT 07", 0, 34);
  g.restore();
  g.fillStyle = "rgba(24,28,30,0.7)";
  g.font = "600 18px ui-monospace, monospace";
  g.fillText("CAUTION \xB7 CRYOGENIC", 60, 236);
  g.fillText("LOX / CH4 \xB7 24 MPa", 60, 262);
  g.fillText("NO STEP", 812, 236);
  g.strokeStyle = "rgba(30,36,38,0.8)";
  g.lineWidth = 3;
  g.beginPath();
  g.arc(180, 540, 34, 0, Math.PI * 2);
  g.stroke();
  g.beginPath();
  g.arc(180, 540, 18, 0, Math.PI * 2);
  g.stroke();
  g.fillStyle = "rgba(30,36,38,0.8)";
  g.fillRect(150, 532, 60, 5);
  const soot = g.createLinearGradient(0, 1024, 0, 840);
  soot.addColorStop(0, "rgba(34,30,26,0.62)");
  soot.addColorStop(1, "rgba(34,30,26,0)");
  g.fillStyle = soot;
  g.fillRect(0, 840, 1024, 184);
  for (let i = 0; i < 70; i++) {
    g.strokeStyle = `rgba(40,38,34,${0.02 + rnd() * 0.05})`;
    g.lineWidth = 0.8 + rnd() * 2.4;
    const x = rnd() * 1024;
    g.beginPath();
    g.moveTo(x, 800 + rnd() * 60);
    g.lineTo(x + (rnd() - 0.5) * 30, 1024);
    g.stroke();
  }
  return finish(c);
}
export function metalTexture() {
  const [c, g] = canvas(512);
  const rnd = mulberry32(20974);
  g.fillStyle = "#2a2e31";
  g.fillRect(0, 0, 512, 512);
  for (let i = 0; i < 700; i++) {
    const v = 20 + Math.floor(rnd() * 40);
    g.fillStyle = `rgba(${v},${v + 2},${v + 3},0.25)`;
    g.fillRect(rnd() * 512, rnd() * 512, 1 + rnd() * 30, 1 + rnd() * 3);
  }
  g.strokeStyle = "rgba(140,150,156,0.20)";
  g.lineWidth = 1.5;
  for (let i = 0; i < 512; i += 64) {
    g.beginPath();
    g.moveTo(i, 0);
    g.lineTo(i, 512);
    g.stroke();
  }
  return finish(c);
}
export function glowTexture(
  inner = "rgba(255,255,255,1)",
  outer = "rgba(64,140,255,0)",
  mid = "rgba(150,200,255,0.20)",
) {
  const [c, g] = canvas(256);
  const grad = g.createRadialGradient(128, 128, 0, 128, 128, 128);
  grad.addColorStop(0, inner);
  grad.addColorStop(0.18, "rgba(255,255,255,0.75)");
  grad.addColorStop(0.5, mid);
  grad.addColorStop(1, outer);
  g.fillStyle = grad;
  g.fillRect(0, 0, 256, 256);
  return finish(c);
}
export function markerTexture(label, color = "#8ff3ff") {
  const c = document.createElement("canvas");
  c.width = 256;
  c.height = 96;
  const g = c.getContext("2d");
  if (!g) throw new Error("2D canvas is unavailable");
  g.clearRect(0, 0, 256, 96);
  g.strokeStyle = color;
  g.lineWidth = 3;
  g.strokeRect(4, 4, 248, 88);
  g.fillStyle = "rgba(4,10,14,0.72)";
  g.fillRect(4, 4, 248, 88);
  g.fillStyle = color;
  g.font = "700 46px ui-monospace, monospace";
  g.textAlign = "center";
  g.textBaseline = "middle";
  g.fillText(label, 128, 52);
  return finish(c);
}
export function starDotTexture() {
  const c = document.createElement("canvas");
  c.width = 32;
  c.height = 32;
  const g = c.getContext("2d");
  if (!g) throw new Error("2D canvas is unavailable");
  const grad = g.createRadialGradient(16, 16, 0, 16, 16, 16);
  grad.addColorStop(0, "rgba(255,255,255,1)");
  grad.addColorStop(0.25, "rgba(255,255,255,0.8)");
  grad.addColorStop(1, "rgba(255,255,255,0)");
  g.fillStyle = grad;
  g.fillRect(0, 0, 32, 32);
  return finish(c, false);
}
export function solarTexture() {
  const [c, g] = canvas(512);
  g.fillStyle = "#0a1526";
  g.fillRect(0, 0, 512, 512);
  const cell = 64;
  for (let y = 0; y < 512; y += cell) {
    for (let x = 0; x < 512; x += cell) {
      const v = 24 + Math.floor(mulberry32(x * 92821 + y * 68917)() * 14);
      g.fillStyle = `rgb(${v},${v + 21},${v + 55})`;
      g.fillRect(x + 2, y + 2, cell - 4, cell - 4);
      g.strokeStyle = "rgba(120,150,190,0.25)";
      g.lineWidth = 1;
      g.strokeRect(x + 2, y + 2, cell - 4, cell - 4);
      g.beginPath();
      g.moveTo(x + 8, y + cell / 2);
      g.lineTo(x + cell - 8, y + cell / 2);
      g.strokeStyle = "rgba(180,200,220,0.12)";
      g.stroke();
    }
  }
  g.strokeStyle = "rgba(201,162,74,0.55)";
  g.lineWidth = 6;
  g.strokeRect(3, 3, 506, 506);
  return finish(c);
}
