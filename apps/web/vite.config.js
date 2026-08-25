import { defineConfig } from "vite";

export default defineConfig({
  // GitHub Pages serves this as a project site under /<repo>/; CI sets
  // BASE_PATH accordingly. Local dev and preview stay at the root.
  base: process.env.BASE_PATH ?? "/",
  optimizeDeps: {
    // Pre-bundling would break the glue code's
    // `new URL("sconote_bg.wasm", import.meta.url)` resolution; verovio is
    // a 7 MB emscripten module with the same import.meta.url pattern.
    exclude: ["@sconote/web", "verovio"],
  },
  build: {
    // Don't inline small assets as data: URLs - audioWorklet.addModule
    // rejects them in some browsers (the worklet must stay a real file).
    assetsInlineLimit: 0,
  },
});
