// Vite bundles only these local SVG glyphs (Lucide, ISC).
const files = import.meta.glob(
  "../node_modules/lucide-static/icons/{crosshair,joystick,orbit,flame,boxes,graduation-cap,telescope,keyboard,sliders-horizontal,fast-forward,play,pause,fingerprint,layers-3,chevron-right,video,minus,plus,scan,compass,power,terminal,clipboard-list,arrow-up-right,x,check,activity,flag,rotate-ccw,settings-2,book-open,download}.svg",
  { eager: true, query: "?raw", import: "default" },
);
const svg = Object.fromEntries(
  Object.entries(files).map(([p, s]) => [
    p.split("/").pop().slice(0, -4),
    s
      .slice(s.indexOf("<svg"))
      .replace('class="lucide', 'aria-hidden="true" class="icon lucide'),
  ]),
);
export function icons(root = document) {
  for (const el of root.querySelectorAll("i[data-i]"))
    el.outerHTML = svg[el.dataset.i] || "";
}
