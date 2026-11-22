import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";
import pkg from "./package.json";

// Tauri expects a fixed port in development and the build in dist/.
export default defineConfig({
  plugins: [react()],
  define: { __APP_VERSION__: JSON.stringify(pkg.version), __APP_DATE__: JSON.stringify(pkg.releaseDate) },
  clearScreen: false,
  server: { port: 1421, strictPort: true, watch: { ignored: ["**/src-tauri/**", "**/target/**"] } },
  build: { target: "es2022", outDir: "dist" },
  test: { include: ["src/**/*.test.{ts,tsx}"], css: true },
});
