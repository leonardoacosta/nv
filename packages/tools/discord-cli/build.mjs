import * as esbuild from "esbuild";
import { readFileSync, writeFileSync } from "fs";

await esbuild.build({
  entryPoints: ["src/index.ts"],
  bundle: true,
  platform: "node",
  target: "node20",
  format: "esm",
  outfile: "dist/discord-cli.js",
  external: [],
});

// Prepend shebang so the file is directly executable
const dist = readFileSync("dist/discord-cli.js", "utf8");
writeFileSync("dist/discord-cli.js", "#!/usr/bin/env node\n" + dist);
console.log("Built dist/discord-cli.js");
