import { execFile } from "node:child_process";

/** Lines containing these substrings are stripped from SSH stdout. */
const NOISE_PATTERNS = ["WARNING:", "vulnerable", "upgraded", "security fix"];

/** SSH stderr patterns that indicate the CloudPC is unreachable. */
const UNREACHABLE_PATTERNS = [
  "Connection refused",
  "timed out",
  "No route to host",
];

/** Transient error patterns worth retrying (CredRead, token expiry, network blip). */
const RETRYABLE_PATTERNS = [
  "CredRead",
  "error 1312",
  "AADSTS",
  "connection reset",
  "broken pipe",
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
  return sshExec(host, cmd, timeoutMs);
}

/** Azure DevOps API resource ID used to acquire AAD tokens. */
const ADO_RESOURCE = "499b84ac-1321-427f-aa17-267ca6975798";

/**
 * Run an Azure DevOps CLI command on the CloudPC via SSH.
 *
 * Unlike sshCloudPC (which dot-sources a pre-existing script), this function
 * generates inline PowerShell that:
 * 1. Acquires a fresh AAD token via `az account get-access-token`
 * 2. Sets AZURE_DEVOPS_EXT_PAT (bypasses Windows CredRead entirely)
 * 3. Executes the provided `az devops` command
 *
 * Includes one automatic retry for transient failures (CredRead, token expiry,
 * network blips). This eliminates the dependency on graph-ado.ps1 and survives
 * CloudPC logon-session expiration (CredRead error 1312).
 */
export function sshAdoCommand(
  host: string,
  adoCommand: string,
  timeoutMs: number = 45_000,
): Promise<string> {
  // Build inline PowerShell that acquires token then runs the command.
  // Using -NoProfile for faster startup; -ExecutionPolicy Bypass for unsigned scripts.
  const ps = [
    `$token = az account get-access-token --resource ${ADO_RESOURCE} --query accessToken -o tsv 2>$null`,
    `if (-not $token) { Write-Error 'Failed to acquire AAD token — run az login on CloudPC'; exit 1 }`,
    `$env:AZURE_DEVOPS_EXT_PAT = $token`,
    adoCommand,
  ].join("; ");

  const cmd = `powershell -NoProfile -ExecutionPolicy Bypass -Command "${ps}"`;
  return sshExecWithRetry(host, cmd, timeoutMs);
}

/**
 * SSH exec with one automatic retry for transient failures.
 * Only retries on patterns known to be transient (CredRead, token, network).
 * Non-retryable errors (unreachable, timeout, script logic) fail immediately.
 */
async function sshExecWithRetry(
  host: string,
  cmd: string,
  timeoutMs: number,
  retries: number = 1,
): Promise<string> {
  try {
    return await sshExec(host, cmd, timeoutMs);
  } catch (err) {
    if (retries <= 0 || !(err instanceof SshError)) throw err;
    // Only retry transient failures, not unreachable (503) or timeout (504)
    if (err.httpStatus !== 502) throw err;
    const isRetryable = RETRYABLE_PATTERNS.some((p) =>
      err.message.toLowerCase().includes(p.toLowerCase()),
    );
    if (!isRetryable) throw err;
    // Brief pause before retry
    await new Promise((r) => setTimeout(r, 2_000));
    return sshExecWithRetry(host, cmd, timeoutMs, retries - 1);
  }
}

/**
 * Low-level SSH execution. Shared by sshCloudPC and sshAdoCommand.
 */
function sshExec(
  host: string,
  cmd: string,
  timeoutMs: number,
): Promise<string> {
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
