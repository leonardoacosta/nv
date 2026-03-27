import type { ServiceConfig } from "../config.js";
import { sshCloudPC } from "../ssh.js";

const OUTLOOK_SCRIPT = "graph-outlook.ps1";

/**
 * Sanitize a user-supplied string before passing it to SSH/PowerShell.
 * Strips single quotes, semicolons, backticks, and pipe characters to prevent injection.
 */
function sanitize(value: string): string {
  return value.replace(/[';`|]/g, "");
}

/**
 * Get recent emails from Outlook inbox.
 * @param limit Number of emails to return (1-50, default 10)
 */
export async function outlookInbox(
  config: ServiceConfig,
  limit: number = 10,
): Promise<string> {
  return sshCloudPC(
    config.cloudpcHost,
    config.cloudpcUserPath,
    OUTLOOK_SCRIPT,
    `-Action Inbox -Count ${limit}`,
  );
}

/**
 * Read the full content of an email by message ID.
 */
export async function outlookRead(
  config: ServiceConfig,
  messageId: string,
): Promise<string> {
  return sshCloudPC(
    config.cloudpcHost,
    config.cloudpcUserPath,
    OUTLOOK_SCRIPT,
    `-Action Read -MessageId '${sanitize(messageId)}'`,
  );
}

/**
 * Search Outlook emails by keyword.
 * @param query Search query string
 * @param limit Number of results to return (1-50, default 10)
 */
export async function outlookSearch(
  config: ServiceConfig,
  query: string,
  limit: number = 10,
): Promise<string> {
  return sshCloudPC(
    config.cloudpcHost,
    config.cloudpcUserPath,
    OUTLOOK_SCRIPT,
    `-Action Search -Query '${sanitize(query)}' -Count ${limit}`,
  );
}

/**
 * List mail folders (Inbox, Sent, Drafts, etc.).
 */
export async function outlookFolders(
  config: ServiceConfig,
): Promise<string> {
  return sshCloudPC(
    config.cloudpcHost,
    config.cloudpcUserPath,
    OUTLOOK_SCRIPT,
    `-Action Folders`,
  );
}

/**
 * Get recent sent emails.
 * @param limit Number of emails to return (1-50, default 10)
 */
export async function outlookSent(
  config: ServiceConfig,
  limit: number = 10,
): Promise<string> {
  return sshCloudPC(
    config.cloudpcHost,
    config.cloudpcUserPath,
    OUTLOOK_SCRIPT,
    `-Action Sent -Count ${limit}`,
  );
}

/**
 * Read emails from a specific folder.
 * @param folderId The Outlook folder ID
 * @param limit Number of emails to return (1-50, default 10)
 */
export async function outlookFolder(
  config: ServiceConfig,
  folderId: string,
  limit: number = 10,
): Promise<string> {
  return sshCloudPC(
    config.cloudpcHost,
    config.cloudpcUserPath,
    OUTLOOK_SCRIPT,
    `-Action Folder -FolderId '${sanitize(folderId)}' -Count ${limit}`,
  );
}
