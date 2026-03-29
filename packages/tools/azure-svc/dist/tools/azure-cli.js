import { sshCloudPC } from "../ssh.js";
import { createLogger } from "../logger.js";
const log = createLogger("azure-cli");
/** Shell metacharacters to strip from user input. */
const DANGEROUS_CHARS = /[;|&$`()><]/g;
/** Destructive flags that warrant a warning (but are not blocked). */
const DESTRUCTIVE_FLAGS = ["--delete", "--purge", "--force-delete"];
/**
 * Sanitize an az command string:
 * - Must start with "az "
 * - Strip dangerous shell metacharacters
 */
function sanitizeCommand(command) {
    const trimmed = command.trim();
    if (!trimmed.startsWith("az ")) {
        throw new Error('Command must start with "az ". Example: az vm list --resource-group myRG');
    }
    // Strip the "az " prefix — we'll add it back in the SSH command
    const args = trimmed.slice(3).replace(DANGEROUS_CHARS, "");
    // Log warnings for destructive flags
    for (const flag of DESTRUCTIVE_FLAGS) {
        if (args.includes(flag)) {
            log.warn({ flag, command: trimmed }, "Destructive flag detected in az command");
        }
    }
    return args;
}
/**
 * Run an Azure CLI command on the CloudPC via SSH.
 * Returns the raw JSON output from az CLI.
 */
export async function runAzureCli(config, command) {
    const sanitizedArgs = sanitizeCommand(command);
    log.info({ command: `az ${sanitizedArgs}` }, "Executing az command on CloudPC");
    // If the user already specified --output, respect it; otherwise default to json
    const hasOutputFlag = sanitizedArgs.includes("--output ") || sanitizedArgs.includes("-o ");
    const outputSuffix = hasOutputFlag ? "" : " --output json";
    const sshCommand = `az ${sanitizedArgs}${outputSuffix}`;
    const result = await sshCloudPC(config.cloudpcHost, sshCommand);
    log.info({ resultLength: result.length }, "az command completed");
    return result;
}
/**
 * Register the azure_cli tool in the tool registry.
 */
export function registerAzureTools(config) {
    return [
        {
            name: "azure_cli",
            description: "Run any Azure CLI command. Authenticated and ready to use. " +
                "Pass the full command including 'az' prefix " +
                "(e.g. 'az vm list', 'az group list', 'az account show'). " +
                "Returns JSON output by default. All Azure operations are available.",
            inputSchema: {
                type: "object",
                properties: {
                    command: {
                        type: "string",
                        description: "The full Azure CLI command to run, starting with 'az'. " +
                            "Examples: 'az vm list', 'az group list --output table', " +
                            "'az account show', 'az resource list --resource-group myRG'.",
                    },
                },
                required: ["command"],
                additionalProperties: false,
            },
            handler: async (input) => {
                const command = input["command"];
                if (typeof command !== "string" || !command.trim()) {
                    throw new Error("Missing required 'command' parameter. Example: az vm list");
                }
                return runAzureCli(config, command);
            },
        },
    ];
}
//# sourceMappingURL=azure-cli.js.map