import { execFile } from "node:child_process";
import { createLogger } from "./logger.js";

const log = createLogger("meta-svc:code-tools");

/** Maximum execution time for typecheck/build commands (2 minutes). */
const TIMEOUT_MS = 120_000;

/** Maximum output size to return (64 KB). */
const MAX_OUTPUT_BYTES = 64 * 1024;

export interface CodeToolResult {
  success: boolean;
  output: string;
  errors: number;
  durationMs: number;
}

function truncate(text: string): string {
  if (Buffer.byteLength(text, "utf-8") <= MAX_OUTPUT_BYTES) return text;
  const truncated = text.slice(0, MAX_OUTPUT_BYTES);
  return truncated + "\n... (output truncated)";
}

function countTsErrors(output: string): number {
  // Match TypeScript error pattern: "file.ts(line,col): error TS..."
  const matches = output.match(/error TS\d+/g);
  return matches ? matches.length : 0;
}

function runCommand(
  command: string,
  args: string[],
): Promise<{ stdout: string; stderr: string; exitCode: number }> {
  return new Promise((resolve) => {
    execFile(
      command,
      args,
      {
        timeout: TIMEOUT_MS,
        maxBuffer: MAX_OUTPUT_BYTES * 2,
        cwd: process.env["NOVA_INSTALL_DIR"] ?? process.cwd(),
        env: { ...process.env, FORCE_COLOR: "0" },
      },
      (error, stdout, stderr) => {
        const exitCode =
          error && "code" in error && typeof error.code === "number"
            ? error.code
            : error
              ? 1
              : 0;
        resolve({ stdout, stderr, exitCode });
      },
    );
  });
}

/**
 * Run `pnpm typecheck` (tsc --noEmit) on a package or the whole workspace.
 */
export async function runTypecheck(
  pkg?: string,
): Promise<CodeToolResult> {
  const start = Date.now();
  const args = pkg
    ? ["--filter", pkg, "typecheck"]
    : ["typecheck"];

  log.info({ package: pkg ?? "(workspace)" }, "Running typecheck");

  const { stdout, stderr, exitCode } = await runCommand("pnpm", args);
  const combined = (stdout + "\n" + stderr).trim();
  const errors = countTsErrors(combined);
  const durationMs = Date.now() - start;

  log.info(
    { package: pkg ?? "(workspace)", success: exitCode === 0, errors, durationMs },
    "Typecheck complete",
  );

  return {
    success: exitCode === 0,
    output: truncate(combined),
    errors,
    durationMs,
  };
}

/**
 * Run `pnpm build` on a package or the whole workspace.
 */
export async function runBuild(
  pkg?: string,
): Promise<CodeToolResult> {
  const start = Date.now();
  const args = pkg
    ? ["--filter", pkg, "build"]
    : ["build"];

  log.info({ package: pkg ?? "(workspace)" }, "Running build");

  const { stdout, stderr, exitCode } = await runCommand("pnpm", args);
  const combined = (stdout + "\n" + stderr).trim();
  const errors = countTsErrors(combined);
  const durationMs = Date.now() - start;

  log.info(
    { package: pkg ?? "(workspace)", success: exitCode === 0, errors, durationMs },
    "Build complete",
  );

  return {
    success: exitCode === 0,
    output: truncate(combined),
    errors,
    durationMs,
  };
}
