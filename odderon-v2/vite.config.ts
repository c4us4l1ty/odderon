import { defineConfig } from "vite";

export default defineConfig({
  // Relative base so the same build loads from a web server, a file:// URL in
  // Electron, or a Steam depot without rewriting paths.
  base: "./",
  server: {
    host: "0.0.0.0",
    port: 5173,
    strictPort: true,
    allowedHosts: true,
  },
  preview: {
    host: "0.0.0.0",
    port: 4173,
    allowedHosts: true,
  },
  build: {
    target: "es2022",
    assetsInlineLimit: 0,
    sourcemap: false,
    // A classic IIFE bundle keeps the electron file:// build working: Chromium
    // refuses module scripts from a file origin.
    rollupOptions: {
      output: {
        format: "iife",
        inlineDynamicImports: true,
        entryFileNames: "assets/[name]-[hash].js",
      },
    },
  },
});
