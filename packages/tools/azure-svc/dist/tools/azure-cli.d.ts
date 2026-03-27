import type { ToolDefinition } from "../tools.js";
import type { ServiceConfig } from "../config.js";
/**
 * Run an Azure CLI command on the CloudPC via SSH.
 * Returns the raw JSON output from az CLI.
 */
export declare function runAzureCli(config: ServiceConfig, command: string): Promise<string>;
/**
 * Register the azure_cli tool in the tool registry.
 */
export declare function registerAzureTools(config: ServiceConfig): ToolDefinition[];
