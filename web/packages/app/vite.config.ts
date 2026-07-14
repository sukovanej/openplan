import tailwindcss from "@tailwindcss/vite"
import react from "@vitejs/plugin-react"
import path from "node:path"
import { defineConfig } from "vite"

const daemon = process.env.OPLAN_DAEMON ?? "http://localhost:7373"
const proxy = { target: daemon, changeOrigin: true }

export default defineConfig({
  plugins: [react(), tailwindcss()],
  resolve: {
    alias: { "@": path.resolve(import.meta.dirname, "src") },
  },
  server: {
    port: 5173,
    proxy: {
      "/api": proxy,
      "/health": proxy,
    },
  },
  build: {
    outDir: "dist",
    emptyOutDir: true,
  },
})
