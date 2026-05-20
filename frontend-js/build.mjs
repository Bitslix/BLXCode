import { build } from "esbuild";

await build({
  entryPoints: ["graph3d_entry.mjs"],
  bundle: true,
  format: "esm",
  platform: "browser",
  target: ["es2020"],
  outfile: "../public/graph3d.bundle.mjs",
  minify: true,
  sourcemap: false,
});
