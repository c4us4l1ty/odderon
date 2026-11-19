// Stage only browser assets. Never stage a worker adapter, remote transport,
// credentials, Wrangler config, or a server binary into the game depot.
import { cp, mkdir, readFile, writeFile } from "node:fs/promises";
import { basename } from "node:path";
import { fileURLToPath } from "node:url";
process.chdir(fileURLToPath(new URL("..", import.meta.url)));
const dest = "public/stations";
// Preserve the directory inode: a running Vite dev server watches this tree.
await mkdir(dest, { recursive: true });
await cp("../odderon-v1/web", dest, {
  recursive: true,
  filter: (p) =>
    ![
      "odd_view.wasm",
      "authority.js",
      "authority-config.js",
      "index.html",
      "title.css",
      "game.js",
      "title-orbit.jpg",
    ].includes(basename(p)),
});
await cp("runtime/odd_view.wasm", `${dest}/odd_view.wasm`);
await cp("src/local-authority.js", `${dest}/authority.js`);
await writeFile(
  `${dest}/authority-config.js`,
  "export const SERVER_PUBLIC_KEY='';\n",
);
for (const page of ["observatory", "mission"]) {
  const path = `${dest}/${page}.html`;
  let html = await readFile(path, "utf8");
  html = html.replace(
    "</head>",
    '<link rel="stylesheet" href="./station.css"></head>',
  );
  await writeFile(path, html);
}
await cp("src/station.css", `${dest}/station.css`);
await cp("../odderon-v1/data/attribution", "public/notices", {
  recursive: true,
});
await cp("node_modules/lucide-static/LICENSE", "public/notices/Lucide.txt");
await cp("node_modules/three/LICENSE", "public/notices/Three.txt");
