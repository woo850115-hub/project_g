import { defineConfig } from "vite";

export default defineConfig({
  server: {
    proxy: {
      "/ws": {
        target: "ws://localhost:4001",
        ws: true,
      },
    },
  },
  build: {
    outDir: "../rust_mud_engine/web_dist",
    emptyOutDir: true,
  },
});
