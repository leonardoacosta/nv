import { execFile } from "node:child_process";
import { mkdirSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { createLogger } from "./logger.js";

const log = createLogger("ssh");

/** Lines containing these substrings are stripped from SSH stdout. */
const NOISE_PATTERNS = ["WARNING:", "vulnerable", "upgraded", "security fix"];

/** SSH stderr patterns that indicate the CloudPC is unreachable. */
const UNREACHABLE_PATTERNS = [
  "Connection refused",
  "timed out",
  "No route to host",
];

/** Default execution timeout: 5 minutes (az vm run-command can take 60-90s). */
const DEFAULT_TIMEOUT_MS = 300_000;

/** Directory for SSH ControlMaster sockets. */
const CONTROL_DIR = join(tmpdir(), "nova-ssh");

/** ControlMaster socket path pattern — one persistent connection per host. */
function controlPath(host: string): string {
  return join(CONTROL_DIR, `%r@${host}:%p`);
}

// Ensure control socket directory exists at module load
try {
  mkdirSync(CONTROL_DIR, { recursive: true, mode: 0o700 });
} catch {
  // Non-fatal — SSH will fall back to direct connections
}

export class SshError extends Error {
  constructor(
    message: string,
    public readonly httpStatus: 502 | 503 | 504,
  ) {
    super(message);
    this.name = "SshError";
  }
}

/**
 * Run a command on the CloudPC via SSH.
 *
 * - Uses ControlMaster for connection reuse (first call opens, subsequent reuse).
 * - 10-second SSH connect timeout, 300-second (5 min) execution timeout.
 * - Filters noise lines from stdout.
 * - Classifies errors into 503 (unreachable), 502 (command error), 504 (timeout).
 * - Logs command metrics: duration, response size, success/failure.
 */
export function sshCloudPC(
  host: string,
  command: string,
): Promise<string> {
  const startMs = Date.now();
  const cmdPreview = command.slice(0, 100);

  return new Promise((resolve, reject) => {
    const child = execFile(
      "ssh",
      [
        "-o", "ConnectTimeout=10",
        "-o", `ControlPath=${controlPath(host)}`,
        "-o", "ControlMaster=auto",
        "-o", "ControlPersist=300",
        host,
        command,
      ],
      { timeout: DEFAULT_TIMEOUT_MS },
      (error, stdout, stderr) => {
        const durationMs = Date.now() - startMs;

        if (error) {
          // Timeout: child_process sets error.killed when the process was killed by timeout
          if (error.killed || error.code === "ETIMEDOUT") {
            log.error(
              { host, command: cmdPreview, durationMs, error: "timeout" },
              `SSH timed out after ${Math.round(DEFAULT_TIMEOUT_MS / 1000)}s`,
            );
            return reject(
              new SshError(
                `CloudPC SSH timed out after ${Math.round(DEFAULT_TIMEOUT_MS / 1000)} seconds`,
                504,
              ),
            );
          }

          const stderrStr = stderr ?? "";

          // Connection failure
          if (
            UNREACHABLE_PATTERNS.some((pattern) =>
              stderrStr.includes(pattern),
            )
          ) {
            log.error(
              { host, command: cmdPreview, durationMs, error: "unreachable" },
              "CloudPC unreachable",
            );
            return reject(
              new SshError(
                "CloudPC unreachable -- cannot connect to 'cloudpc' via SSH",
                503,
              ),
            );
          }

          // Command error (non-zero exit)
          const errMsg = stderrStr.trim() || error.message;
          log.warn(
            { host, command: cmdPreview, durationMs, error: errMsg.slice(0, 200) },
            "SSH command failed",
          );
          return reject(
            new SshError(errMsg, 502),
          );
        }

        // Filter noise lines from stdout
        const filtered = (stdout ?? "")
          .split("\n")
          .filter(
            (line) =>
              !NOISE_PATTERNS.some((pattern) => line.includes(pattern)),
          )
          .join("\n")
          .trim();

        log.info(
          {
            host,
            command: cmdPreview,
            durationMs,
            responseBytes: filtered.length,
            responseLines: filtered.split("\n").length,
          },
          "SSH command completed",
        );

        resolve(filtered);
      },
    );

    // Safety net: if the child process somehow ignores the timeout option
    child.on("error", (err) => {
      const durationMs = Date.now() - startMs;
      log.error(
        { host, command: cmdPreview, durationMs, error: err.message },
        "SSH child process error",
      );
      reject(new SshError(err.message, 502));
    });
  });
}
