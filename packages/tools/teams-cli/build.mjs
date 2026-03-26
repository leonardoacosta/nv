import * as esbuild from "esbuild";
import { readFileSync, writeFileSync } from "fs";

await esbuild.build({
  entryPoints: ["src/index.ts"],
  bundle: true,
  platform: "node",
  target: "node20",
  format: "cjs",
  outfile: "dist/teams-cli.cjs",
  external: [],
});

// Prepend shebang so the file is directly executable
const dist = readFileSync("dist/teams-cli.cjs", "utf8");
writeFileSync("dist/teams-cli.cjs", "#!/usr/bin/env node\n" + dist);
console.log("Built dist/teams-cli.cjs");
