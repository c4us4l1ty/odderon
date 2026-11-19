/* Desktop shell for the Steam build.
 *
 * The renderer is the same static Vite bundle served to browsers. No Node
 * integration is exposed, so the desktop build is just the game in a window. */

const { app, BrowserWindow, shell } = require("electron");
const path = require("node:path");

const isDev = !app.isPackaged && process.env.ODDERON_DEV === "1";

function createWindow() {
  const win = new BrowserWindow({
    width: 1600,
    height: 900,
    minWidth: 1024,
    minHeight: 640,
    backgroundColor: "#010305",
    show: false,
    autoHideMenuBar: true,
    fullscreenable: true,
    title: "ODDERON",
    webPreferences: {
      contextIsolation: true,
      nodeIntegration: false,
      sandbox: true,
      backgroundThrottling: false,
    },
  });

  win.setMenuBarVisibility(false);
  win.once("ready-to-show", () => {
    win.show();
    if (process.env.ODDERON_FULLSCREEN !== "0") win.setFullScreen(true);
  });

  if (isDev) {
    void win.loadURL("http://localhost:5173/");
  } else {
    void win.loadFile(path.join(__dirname, "..", "dist", "index.html"));
  }

  // Never navigate the game window away from the bundle.
  win.webContents.setWindowOpenHandler(({ url }) => {
    void shell.openExternal(url);
    return { action: "deny" };
  });

  return win;
}

app.whenReady().then(() => {
  createWindow();
  app.on("activate", () => {
    if (BrowserWindow.getAllWindows().length === 0) createWindow();
  });
});

app.on("window-all-closed", () => {
  if (process.platform !== "darwin") app.quit();
});
