import { defineConfig } from "tsup";

export default defineConfig({
  entry: {
    index: "index.ts",
    "client-kit": "client-kit.ts",
    "client-legacy": "client-legacy.ts",
  },
  format: ["cjs", "esm"],
  dts: true,
  splitting: false,
  sourcemap: true,
  clean: true,
  treeshake: true,
  target: "es2022",
  outDir: "dist",
});
