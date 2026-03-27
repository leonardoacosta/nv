/** Child process exec wrapper with timeout. */
interface ExecResult {
    stdout: string;
    stderr: string;
    exitCode: number;
}
/**
 * Run a command via execFile with a timeout.
 * Returns { stdout, stderr, exitCode } - never throws.
 */
export declare function exec(cmd: string, args: string[], timeoutMs?: number): Promise<ExecResult>;
/**
 * Spawn a command that replaces the current process (for log tailing).
 * Uses execFile in passthrough mode.
 */
export declare function execPassthrough(cmd: string, args: string[]): void;
export {};
