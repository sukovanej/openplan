import tailwindcss from "@tailwindcss/vite"
import react from "@vitejs/plugin-react"
import { defineConfig, type ProxyOptions } from "vite"

const daemon = process.env.OPENPLAN_DAEMON ?? "http://localhost:7373"
const proxy: ProxyOptions = {
  target: daemon,
  changeOrigin: true,
  configure: (server) => {
    // Node holds response headers back until the first body chunk, and the daemon writes none to
    // the event stream until an event happens. The UI shows the daemon as down until the stream
    // opens, so the headers must go out at once.
    server.on("proxyRes", (upstream, _request, response) => {
      if (upstream.headers["content-type"] !== "text/event-stream") return
      response.writeHead(upstream.statusCode ?? 200, upstream.headers)
      response.flushHeaders()
    })
  },
}

export default defineConfig({
  plugins: [react(), tailwindcss()],
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
