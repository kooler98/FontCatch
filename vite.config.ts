import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";
import tailwindcss from "@tailwindcss/vite";
export default defineConfig({
  plugins: [react(), tailwindcss()],
  clearScreen: false,
  server: { watch: { ignored: ["**/src-tauri/**", "**/.tooling/**"] } },
  build: { target: ["es2022", "safari14"] },
});
