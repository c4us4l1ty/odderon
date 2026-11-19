import { defineConfig } from "vite";
export default defineConfig({
  base: "./",
  // The historical games have separate entry points and dependencies.
  optimizeDeps: { entries: ["index.html"] },
  server: { host: "0.0.0.0", allowedHosts: [".e2b.app"] },
  preview: { host: "0.0.0.0", allowedHosts: [".e2b.app"] },
});
