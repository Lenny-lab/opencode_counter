import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";
import tailwindcss from "@tailwindcss/vite";
import path from "node:path";

// Tauri drives the dev server on a fixed port and needs a stable host.
const host = process.env.TAURI_DEV_HOST;

export default defineConfig({
  plugins: [react(), tailwindcss()],
  resolve: {
    alias: { "@": path.resolve(import.meta.dirname, "./src") },
  },
  clearScreen: false,
  server: {
    port: 5183,
    strictPort: true,
    host: host || false,
    hmr: host ? { protocol: "ws", host, port: 5184 } : undefined,
    watch: { ignored: ["**/src-tauri/**"] },
  },
  build: {
    // WebView2 on Windows 10 and 11 both handle the modern syntax; 110 is the
    // oldest baseline that still gets a passable bundle from the minifier.
    target: "chrome110",
    // Vite 8 minifies with oxc, not esbuild. Naming esbuild here would drag in
    // a second bundler engine for no gain.
    minify: true,
    sourcemap: false,
  },
});
