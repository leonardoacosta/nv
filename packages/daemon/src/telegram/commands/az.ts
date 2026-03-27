import { fleetPost } from "../../fleet-client.js";

const TELEGRAM_MAX_CHARS = 4000;
const AZURE_SVC_PORT = 4109;

/**
 * /az [command] -- run an Azure CLI command via azure-svc
 *
 * Usage:
 *   /az vm list
 *   /az group list
 *   /az account show
 */
export async function buildAzReply(command?: string): Promise<string> {
  if (!command) {
    return [
      "Azure CLI",
      "─".repeat(32),
      "Usage: /az <command>",
      "",
      "Examples:",
      "  /az account show",
      "  /az vm list",
      "  /az group list",
      "  /az resource list --resource-group myRG",
      "",
      'Pass any "az" command. The "az" prefix is added automatically if omitted.',
    ].join("\n");
  }

  // Ensure the command starts with "az "
  const fullCommand = command.startsWith("az ") ? command : `az ${command}`;

  const data = (await fleetPost(AZURE_SVC_PORT, "/az", {
    command: fullCommand,
  })) as { result?: string; error?: string };

  if (data.error) {
    return `Azure CLI Error\n${"─".repeat(32)}\n${data.error}`;
  }

  const result = data.result ?? "No output";

  // Try to pretty-format JSON output
  let formatted: string;
  try {
    const parsed = JSON.parse(result) as unknown;
    formatted = JSON.stringify(parsed, null, 2);
  } catch {
    formatted = result;
  }

  return truncate(formatted);
}

function truncate(text: string): string {
  if (text.length <= TELEGRAM_MAX_CHARS) return text;
  return text.slice(0, TELEGRAM_MAX_CHARS - 3) + "...";
}
