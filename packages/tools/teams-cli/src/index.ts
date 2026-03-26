import { Command } from "commander";
import { listChats, readChat } from "./commands/chats.js";
import { listChannels } from "./commands/channels.js";
import { listMessages } from "./commands/messages.js";
import { checkPresence } from "./commands/presence.js";
import { sendMessage } from "./commands/send.js";

const program = new Command();

program
  .name("teams-cli")
  .description("Microsoft Teams CLI — direct Graph API access via client_credentials")
  .version("0.1.0");

program
  .command("chats")
  .description("List recent chats (DMs and group chats)")
  .option("--limit <n>", "Number of chats to return (max 50)", "20")
  .action(async (opts: { limit: string }) => {
    const limit = Math.min(parseInt(opts.limit, 10) || 20, 50);
    await listChats(limit).catch(handleError);
  });

program
  .command("read-chat <id>")
  .description("Read messages from a chat")
  .option("--limit <n>", "Number of messages to return (max 50)", "20")
  .action(async (id: string, opts: { limit: string }) => {
    const limit = Math.min(parseInt(opts.limit, 10) || 20, 50);
    await readChat(id, limit).catch(handleError);
  });

program
  .command("channels <team-id>")
  .description("List channels in a team")
  .action(async (teamId: string) => {
    await listChannels(teamId).catch(handleError);
  });

program
  .command("messages <team-id> <channel-id>")
  .description("Read messages from a team channel")
  .option("--limit <n>", "Number of messages to return (max 50)", "20")
  .action(async (teamId: string, channelId: string, opts: { limit: string }) => {
    const limit = Math.min(parseInt(opts.limit, 10) || 20, 50);
    await listMessages(teamId, channelId, limit).catch(handleError);
  });

program
  .command("presence <user>")
  .description("Check user presence/availability (email or user ID)")
  .action(async (user: string) => {
    await checkPresence(user).catch(handleError);
  });

program
  .command("send <chat-id> <message>")
  .description("Send a message to a chat")
  .action(async (chatId: string, message: string) => {
    await sendMessage(chatId, message).catch(handleError);
  });

function handleError(err: unknown): void {
  if (err instanceof Error) {
    process.stderr.write(`teams-cli error: ${err.message}\n`);
  } else {
    process.stderr.write(`teams-cli error: ${String(err)}\n`);
  }
  process.exit(1);
}

program.parse(process.argv);
