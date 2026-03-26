import { DiscordClient } from "../auth.js";

interface DiscordGuild {
  id: string;
  name: string;
}

export async function guildsCommand(client: DiscordClient): Promise<void> {
  const guilds = (await client.get("/users/@me/guilds")) as DiscordGuild[];

  if (guilds.length === 0) {
    console.log("Bot is not a member of any guilds.");
    return;
  }

  console.log(`Guilds (${guilds.length})`);
  for (const guild of guilds) {
    // Pad ID to fixed width (18 digits) for alignment
    console.log(`${guild.id.padEnd(20)}${guild.name}`);
  }
}
