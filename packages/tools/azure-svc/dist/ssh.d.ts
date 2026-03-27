export declare class SshError extends Error {
    readonly httpStatus: 502 | 503 | 504;
    constructor(message: string, httpStatus: 502 | 503 | 504);
}
/**
 * Run a command on the CloudPC via SSH.
 *
 * - 10-second SSH connect timeout, 60-second execution timeout (az commands can be slow).
 * - Filters noise lines from stdout.
 * - Classifies errors into 503 (unreachable), 502 (command error), 504 (timeout).
 */
export declare function sshCloudPC(host: string, command: string): Promise<string>;
