import { build } from "esbuild";

const shared = {
  bundle: true,
  format: "esm",
  platform: "browser",
  target: ["es2020"],
  minify: true,
  sourcemap: false,
};

await Promise.all([
  build({
    ...shared,
    entryPoints: ["graph3d_entry.mjs"],
    outfile: "../public/graph3d.bundle.mjs",
  }),
  build({
    ...shared,
    entryPoints: ["drobo_orb_entry.mjs"],
    outfile: "../public/drobo_orb.bundle.mjs",
  }),
]);
