import path from "node:path";
import { defineConfig } from "vite";
import react from "@vitejs/plugin-react-swc";
import tailwindcss from "@tailwindcss/vite";
import { codeInspectorPlugin } from "code-inspector-plugin";

const host = process.env.TAURI_DEV_HOST;

export default defineConfig(({ command }) => {
  const plugins = [
    react(),
    tailwindcss(),
  ];

  if (command === "serve") {
    plugins.push(
      codeInspectorPlugin({
        bundler: "vite",
        server: "open",
      }),
    );
  }

  return {
    plugins,
    build: {
      rollupOptions: {
        output: {
          manualChunks: {
            antd: ["antd", "@ant-design/icons"],
            react: ["react", "react-dom", "react-router-dom"],
            query: ["@tanstack/react-query", "zustand"],
          },
        },
      },
    },
    resolve: {
      alias: {
        "@": path.resolve(__dirname, "./src"),
      },
    },
    clearScreen: false,
    server: {
      port: 1420,
      strictPort: true,
      host: host || false,
      hmr: host
        ? {
            protocol: "ws",
            host,
            port: 1421,
          }
        : undefined,
      watch: {
        ignored: ["**/src-tauri/**"],
      },
    },
  };
});
