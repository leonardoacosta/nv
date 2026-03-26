import { Command } from "commander";
import { DiscordClient } from "./auth.js";
import { guildsCommand } from "./commands/guilds.js";
import { channelsCommand } from "./commands/channels.js";
import { messagesCommand } from "./commands/messages.js";

const program = new Command();

program
  .name("discord-cli")
  .description("Discord bot CLI for read operations")
  .version("0.1.0");

program
  .command("guilds")
  .description("List all guilds the bot is a member of")
  .action(async () => {
    const client = new DiscordClient();
    await guildsCommand(client);
  });

program
  .command("channels <guild-id>")
  .description("List text channels in a guild, grouped by category")
  .action(async (guildId: string) => {
    const client = new DiscordClient();
    await channelsCommand(client, guildId);
  });

program
  .command("messages <channel-id>")
  .description("Read recent messages from a channel")
  .option("-l, --limit <number>", "Number of messages to fetch (max 100)", "50")
  .action(async (channelId: string, options: { limit: string }) => {
    const limit = Math.min(100, Math.max(1, parseInt(options.limit, 10)));
    const client = new DiscordClient();
    await messagesCommand(client, channelId, limit);
  });

program.parseAsync(process.argv).catch((err: unknown) => {
  const message = err instanceof Error ? err.message : String(err);
  process.stderr.write(`Error: ${message}\n`);
  process.exit(1);
});
