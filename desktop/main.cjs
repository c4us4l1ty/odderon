// Optional desktop host. One local, secure application origin supports ES
// modules, isolated frames, fetch, storage and WASM workers without an HTTP
// listener. This is NOT a Steamworks integration or a distribution installer.
const { app, BrowserWindow, protocol, net, session } = require("electron");
const path = require("node:path");
const { pathToFileURL } = require("node:url");
const root = path.resolve(__dirname, "../dist"),
  origin = "odderon://game";
protocol.registerSchemesAsPrivileged([
  {
    scheme: "odderon",
    privileges: {
      standard: true,
      secure: true,
      supportFetchAPI: true,
      corsEnabled: true,
      stream: true,
    },
  },
]);
function create() {
  const win = new BrowserWindow({
    width: 1500,
    height: 900,
    minWidth: 720,
    minHeight: 540,
    title: "Odderon III",
    autoHideMenuBar: true,
    backgroundColor: "#0a0e12",
    webPreferences: {
      contextIsolation: true,
      nodeIntegration: false,
      sandbox: true,
    },
  });
  win.webContents.setWindowOpenHandler(() => ({ action: "deny" }));
  win.webContents.on("will-navigate", (event, url) => {
    if (!url.startsWith(origin + "/")) event.preventDefault();
  });
  win.loadURL(origin + "/index.html");
}
app.whenReady().then(() => {
  protocol.handle("odderon", (request) => {
    const url = new URL(request.url);
    if (url.host !== "game") return new Response("Not found", { status: 404 });
    let relative;
    try {
      relative =
        decodeURIComponent(url.pathname).replace(/^\/+/, "") || "index.html";
    } catch {
      return new Response("Bad path", { status: 400 });
    }
    const file = path.resolve(root, relative);
    if (!file.startsWith(root + path.sep))
      return new Response("Not found", { status: 404 });
    return net.fetch(pathToFileURL(file).href);
  });
  session.defaultSession.webRequest.onBeforeRequest(
    { urls: ["http://*/*", "https://*/*", "ws://*/*", "wss://*/*"] },
    (_, cb) => cb({ cancel: true }),
  );
  create();
  app.on("activate", () => {
    if (!BrowserWindow.getAllWindows().length) create();
  });
});
app.on("window-all-closed", () => {
  if (process.platform !== "darwin") app.quit();
});
