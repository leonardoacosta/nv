import type { DiscordClient } from "../client.js";

interface DiscordGuild {
  id: string;
  name: string;
  icon: string | null;
}

export interface GuildsResult {
  guilds: Array<{ id: string; name: string; icon: string | null }>;
}

export async function listGuilds(client: DiscordClient): Promise<GuildsResult> {
  const guilds = (await client.get("/users/@me/guilds")) as DiscordGuild[];

  return {
    guilds: guilds.map((g) => ({
      id: g.id,
      name: g.name,
      icon: g.icon,
    })),
  };
}
