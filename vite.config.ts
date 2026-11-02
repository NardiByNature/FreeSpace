import { defineConfig } from "vite";

export default defineConfig({
  // Relative asset paths so the built dist/ opens straight from a folder.
  base: "./",
  build: {
    target: "es2022",
    sourcemap: true,
  },
  server: {
    fs: {
      // Only the project itself; nothing above it is ever served.
      strict: true,
      allow: ["."],
    },
  },
});
