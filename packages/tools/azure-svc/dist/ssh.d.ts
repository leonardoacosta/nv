export declare class SshError extends Error {
    readonly httpStatus: 502 | 503 | 504;
    constructor(message: string, httpStatus: 502 | 503 | 504);
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
export declare function sshCloudPC(host: string, command: string): Promise<string>;
