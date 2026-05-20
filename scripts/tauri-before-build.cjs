#!/usr/bin/env node
/**
 * Cross-platform beforeBuild / beforeDev hook for Tauri (cwd: src-tauri).
 * Usage: node ../scripts/tauri-before-build.cjs [--dev]
 */
const { spawnSync } = require("child_process");
const path = require("path");

const root = path.resolve(__dirname, "..");
const dev = process.argv.includes("--dev");
const npm = process.platform === "win32" ? "npm.cmd" : "npm";
const pathKey = process.platform === "win32" ? "Path" : "PATH";
const cargoBin = path.join(process.env.HOME || "", ".cargo", "bin");

function commandEnv(cmd) {
  const env = { ...process.env };
  env[pathKey] = `${cargoBin}${path.delimiter}${env[pathKey] || ""}`;
  if (cmd === "trunk" && env.NO_COLOR && env.NO_COLOR !== "true" && env.NO_COLOR !== "false") {
    env.NO_COLOR = "true";
  }
  return env;
}

function run(cmd, args, cwd) {
  const result = spawnSync(cmd, args, {
    cwd,
    stdio: "inherit",
    shell: process.platform === "win32",
    env: commandEnv(cmd),
  });
  if (result.error) {
    console.error(result.error.message);
    process.exit(1);
  }
  if (result.status !== 0) {
    process.exit(result.status ?? 1);
  }
}

run(npm, ["--prefix", path.join(root, "frontend-js"), "run", "build:graph3d"], root);
run("trunk", [dev ? "serve" : "build"], root);
