import { execFile } from "node:child_process";

/** Lines containing these substrings are stripped from SSH stdout. */
const NOISE_PATTERNS = ["WARNING:", "vulnerable", "upgraded", "security fix"];

/** SSH stderr patterns that indicate the CloudPC is unreachable. */
const UNREACHABLE_PATTERNS = [
  "Connection refused",
  "timed out",
  "No route to host",
];

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
 * Run a PowerShell script on the CloudPC via SSH.
 *
 * Mirrors the Rust `cloudpc::ssh_cloudpc_script` function:
 * - Dot-sources the script and invokes it with the provided args.
 * - 10-second SSH connect timeout, 30-second execution timeout.
 * - Filters noise lines from stdout.
 * - Classifies errors into 503 (unreachable), 502 (script error), 504 (timeout).
 */
export function sshCloudPC(
  host: string,
  userPath: string,
  script: string,
  args: string,
  timeoutMs: number = 30_000,
): Promise<string> {
  const cmd = `powershell -ExecutionPolicy Bypass -Command "& { . ${userPath}\\${script} ${args} }"`;

  return new Promise((resolve, reject) => {
    const child = execFile(
      "ssh",
      ["-o", "ConnectTimeout=10", host, cmd],
      { timeout: timeoutMs },
      (error, stdout, stderr) => {
        if (error) {
          // Timeout: child_process sets error.killed when the process was killed by timeout
          if (error.killed || error.code === "ETIMEDOUT") {
            return reject(
              new SshError(
                `CloudPC SSH timed out after ${Math.round(timeoutMs / 1000)} seconds`,
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
            return reject(
              new SshError(
                "CloudPC unreachable -- cannot connect to 'cloudpc' via SSH",
                503,
              ),
            );
          }

          // Script error (non-zero exit)
          return reject(
            new SshError(
              stderrStr.trim() || error.message,
              502,
            ),
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

        resolve(filtered);
      },
    );

    // Safety net: if the child process somehow ignores the timeout option
    child.on("error", (err) => {
      reject(new SshError(err.message, 502));
    });
  });
}
