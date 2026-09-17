import { defineConfig } from "vite";

export default defineConfig({
  publicDir: "public",
  clearScreen: false,
  server: { port: 1422, strictPort: true },
});
