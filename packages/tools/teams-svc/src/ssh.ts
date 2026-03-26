/**
 * CloudPC SSH helper for running PowerShell scripts on the remote Windows machine.
 *
 * All Teams tools execute PowerShell scripts on `cloudpc` via SSH instead of
 * calling Graph API directly. The scripts manage their own device-code + token
 * refresh flow.
 */

import { execFile } from "node:child_process";

const CLOUDPC_HOST = "cloudpc";
const CLOUDPC_USER_PATH = String.raw`C:\Users\leo.346-CPC-QJXVZ`;

/** Lines matching these substrings are noise from SSH/PowerShell and get filtered. */
const NOISE_PATTERNS = ["WARNING:", "vulnerable", "upgraded", "security fix"];

export class CloudPcUnreachableError extends Error {
  constructor() {
    super("CloudPC unreachable -- cannot connect via SSH");
    this.name = "CloudPcUnreachableError";
  }
}

export class CloudPcScriptError extends Error {
  constructor(stderr: string) {
    super(`CloudPC script failed: ${stderr}`);
    this.name = "CloudPcScriptError";
  }
}

/**
 * Run a PowerShell script on the CloudPC via SSH.
 *
 * The script is dot-sourced and invoked with the provided `args` string.
 * Returns stdout as a string with SSH/PowerShell noise lines stripped.
 *
 * @throws {CloudPcUnreachableError} If the SSH connection fails.
 * @throws {CloudPcScriptError} If the PowerShell script exits with a non-zero status.
 */
export function sshCloudPc(script: string, args: string): Promise<string> {
  const cmd = `powershell -ExecutionPolicy Bypass -Command "& { . ${CLOUDPC_USER_PATH}\\${script} ${args} }"`;

  return new Promise((resolve, reject) => {
    execFile(
      "ssh",
      ["-o", "ConnectTimeout=10", CLOUDPC_HOST, cmd],
      { timeout: 30_000 },
      (error, stdout, stderr) => {
        if (error) {
          const stderrStr = stderr ?? "";

          // Connection-level failures
          if (
            stderrStr.includes("Connection refused") ||
            stderrStr.includes("timed out") ||
            stderrStr.includes("No route to host")
          ) {
            reject(new CloudPcUnreachableError());
            return;
          }

          // Timeout from Node's child_process
          if (error.killed) {
            reject(new CloudPcUnreachableError());
            return;
          }

          // Script-level failures
          reject(new CloudPcScriptError(stderrStr || error.message));
          return;
        }

        // Filter noise lines
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
  });
}
