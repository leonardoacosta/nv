import type { ToolDefinition } from "../tools.js";
import type { ServiceConfig } from "../config.js";
import { sshCloudPC } from "../ssh.js";
import { createLogger } from "../logger.js";

const log = createLogger("ssh-command");

/**
 * Run an arbitrary command on the CloudPC via SSH.
 * No sanitization or prefix check — accepts PowerShell, CMD, or any CLI tool.
 */
export async function runSshCommand(
  config: ServiceConfig,
  command: string,
): Promise<string> {
  const cmdPreview = command.slice(0, 120);

  log.info({ command: cmdPreview }, "Executing SSH command on CloudPC");

  const result = await sshCloudPC(config.cloudpcHost, command);

  log.info(
    { resultLength: result.length },
    "SSH command completed",
  );

  return result;
}

/**
 * Register the ssh_command tool in the tool registry.
 */
export function registerSshTools(
  config: ServiceConfig,
): ToolDefinition[] {
  return [
    {
      name: "ssh_command",
      description:
        "Run any command on the CloudPC via SSH. " +
        "Accepts PowerShell, CMD, or any CLI tool available on the machine. " +
        "Returns stdout. Use for diagnostics, file operations, network checks, " +
        "or any task that isn't covered by specialized tools.",
      inputSchema: {
        type: "object",
        properties: {
          command: {
            type: "string",
            description:
              "The command to execute on the CloudPC. " +
              "Examples: 'Get-Process | Sort CPU -Desc', 'ipconfig /all', " +
              "'hostname', 'dir C:\\Users'.",
          },
        },
        required: ["command"],
        additionalProperties: false,
      },
      handler: async (input) => {
        const command = input["command"];
        if (typeof command !== "string" || !command.trim()) {
          throw new Error(
            "Missing required 'command' parameter. " +
            "Example: Get-Process | Sort CPU -Desc",
          );
        }
        return runSshCommand(config, command);
      },
    },
  ];
}
