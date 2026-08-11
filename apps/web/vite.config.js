import { defineConfig } from "vite";

export default defineConfig({
  optimizeDeps: {
    // Pre-bundling would break the glue code's
    // `new URL("sconote_bg.wasm", import.meta.url)` resolution.
    exclude: ["@sconote/web"],
  },
  build: {
    // Don't inline small assets as data: URLs — audioWorklet.addModule
    // rejects them in some browsers (the worklet must stay a real file).
    assetsInlineLimit: 0,
  },
});
