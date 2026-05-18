import path from "node:path";
import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";

const tunnelMode = process.env.EUNOMIA_TUNNEL === "1";

export default defineConfig({
  plugins: [react()],
  resolve: {
    alias: {
      "@": path.resolve(__dirname, "./src"),
    },
  },
  server: {
    host: "127.0.0.1",
    port: 5173,
    proxy: {
      "/api": "http://127.0.0.1:3001",
    },
    allowedHosts: true,
    hmr: tunnelMode ? { clientPort: 443, protocol: "wss" } : true,
  },
});
