/** Child process exec wrapper with timeout. */
import { execFile as execFileCb } from "node:child_process";
/**
 * Run a command via execFile with a timeout.
 * Returns { stdout, stderr, exitCode } - never throws.
 */
export async function exec(cmd, args, timeoutMs = 5000) {
    return new Promise((resolve) => {
        const child = execFileCb(cmd, args, { timeout: timeoutMs, encoding: "utf8" }, (error, stdout, stderr) => {
            const exitCode = error && "code" in error ? error.code ?? 1 : 0;
            resolve({
                stdout: (stdout ?? "").trim(),
                stderr: (stderr ?? "").trim(),
                exitCode,
            });
        });
        // Safety: kill if somehow the callback hasn't fired
        child.unref?.();
    });
}
/**
 * Spawn a command that replaces the current process (for log tailing).
 * Uses execFile in passthrough mode.
 */
export function execPassthrough(cmd, args) {
    const { spawn } = require("node:child_process");
    const child = spawn(cmd, args, { stdio: "inherit" });
    child.on("exit", (code) => process.exit(code ?? 0));
}
//# sourceMappingURL=exec.js.map